//! Rotates pages on filing the way the preview shows them (detected and manually chosen rotation).
//!
//! - PDF: page rotation (`/Rotate`), appended as an incremental update; the content stays unchanged.
//!   Signed and encrypted PDFs are left untouched, as with the tags.
//! - JPEG and PNG: the pixels are saved again, rotated (JPEG in high quality). An EXIF orientation is then part of
//!   the pixels; the other EXIF data is lost.
//! - Other formats stay as they are.

use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use lopdf::{Dictionary, Document, IncrementalDocument, Object, ObjectId};

use crate::i18n::files;
use crate::preprocess;
use crate::tags::{self, Outcome};

/// JPEG quality when saving again: visually lossless, files hardly larger than from the scanner.
const JPEG_QUALITY: u8 = 92;

/// Rotates the pages of the file by `turns[i]` degrees clockwise (multiples of 90). Errors leave the file
/// unchanged.
pub fn apply(path: &Path, turns: &[u16]) -> Result<Outcome> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "pdf" => pdf(path, turns),
        "jpg" | "jpeg" => image(path, turns.first().copied().unwrap_or(0), ImageFormat::Jpeg),
        "png" => image(path, turns.first().copied().unwrap_or(0), ImageFormat::Png),
        _ => Ok(Outcome::Unsupported),
    }
}

/// Is there any rotation that is not just full turns?
pub fn any(turns: &[u16]) -> bool {
    turns.iter().any(|t| t % 360 != 0)
}

fn pdf(path: &Path, turns: &[u16]) -> Result<Outcome> {
    let t = files::t();
    let mut doc = IncrementalDocument::load(path).context(t.pdf_not_read)?;
    let prev = doc.get_prev_documents();
    if prev.is_encrypted() {
        return Ok(Outcome::Skipped(t.skip_encrypted.into()));
    }
    if tags::is_signed(prev) {
        return Ok(Outcome::Skipped(t.skip_signed.into()));
    }
    let pages = prev.get_pages();
    let mut changed: Vec<(ObjectId, Dictionary)> = Vec::new();
    for (i, turn) in turns.iter().enumerate() {
        let turn = i64::from(turn % 360);
        let Some(&id) = pages.get(&(i as u32 + 1)) else { continue };
        if turn == 0 {
            continue;
        }
        let mut page = prev.get_object(id).and_then(Object::as_dict).context(t.pdf_not_read)?.clone();
        let rotate = (page_rotation(prev, &page) + turn).rem_euclid(360);
        page.set("Rotate", Object::Integer(rotate));
        changed.push((id, page));
    }
    if changed.is_empty() {
        return Ok(Outcome::Written);
    }
    for (id, page) in changed {
        doc.new_document.set_object(id, page);
    }
    tags::replace_with(path, |f| {
        let mut out = BufWriter::new(f);
        doc.save_to(&mut out).context(t.pdf_not_written)?;
        out.flush()?;
        Ok(())
    })?;
    Ok(Outcome::Written)
}

/// Current rotation of the page in degrees. `/Rotate` is inherited from the page tree if the page has none.
fn page_rotation(doc: &Document, page: &Dictionary) -> i64 {
    let mut dict = page;
    // Page trees are flat; the limit only guards against cycles in broken files.
    for _ in 0..32 {
        if let Ok(rotate) = dict.get(b"Rotate").and_then(Object::as_i64) {
            return rotate.rem_euclid(360);
        }
        match dict.get(b"Parent").and_then(Object::as_reference).and_then(|id| doc.get_object(id)) {
            Ok(Object::Dictionary(parent)) => dict = parent,
            _ => break,
        }
    }
    0
}

fn image(path: &Path, turn: u16, format: ImageFormat) -> Result<Outcome> {
    let t = files::t();
    let turn = turn % 360;
    if turn == 0 {
        return Ok(Outcome::Written);
    }
    let read = || -> Result<DynamicImage> {
        let mut decoder = ImageReader::open(path)?.with_guessed_format()?.into_decoder()?;
        let orientation = decoder.orientation()?;
        let mut img = DynamicImage::from_decoder(decoder)?;
        img.apply_orientation(orientation);
        Ok(img)
    };
    let img = preprocess::rotate_dynamic(&read().context(t.image_not_read)?, turn);
    tags::replace_with(path, |f| {
        let mut out = BufWriter::new(f);
        match format {
            ImageFormat::Jpeg => {
                // JPEG has no alpha channel.
                let rgb = img.to_rgb8();
                JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).encode_image(&rgb)
            }
            _ => img.write_to(&mut out, format),
        }
        .context(t.image_not_written)?;
        out.flush()?;
        Ok(())
    })?;
    Ok(Outcome::Written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    /// PDF with two empty pages; the page tree sets `/Rotate 90`, page 2 has its own 180°.
    fn two_pages(path: &Path) {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let p1 = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()] });
        let p2 = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "Rotate" => 180, "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()] });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(
                dictionary! { "Type" => "Pages", "Kids" => vec![p1.into(), p2.into()], "Count" => 2, "Rotate" => 90 },
            ),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        doc.save(path).unwrap();
    }

    fn rotations(path: &Path) -> Vec<i64> {
        let doc = Document::load(path).unwrap();
        doc.get_pages()
            .values()
            .map(|id| page_rotation(&doc, doc.get_object(*id).unwrap().as_dict().unwrap()))
            .collect()
    }

    #[test]
    fn pdf_pages_turn_on_top_of_inherited_rotation() {
        let dir = std::env::temp_dir().join(format!("scrinium-rotate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("zwei.pdf");
        two_pages(&path);
        let before = std::fs::read(&path).unwrap();
        assert_eq!(rotations(&path), [90, 180]);

        assert_eq!(apply(&path, &[270, 90]).unwrap(), Outcome::Written);
        assert_eq!(rotations(&path), [0, 270]);
        // Incremental: the old content stays unchanged at the start.
        assert!(std::fs::read(&path).unwrap().starts_with(&before));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn images_are_saved_turned() {
        let dir = std::env::temp_dir().join(format!("scrinium-rotate-img-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hoch.png");
        // 2×3 pixels, red at the top left.
        let mut img = image::RgbImage::from_pixel(2, 3, image::Rgb([255, 255, 255]));
        img.put_pixel(0, 0, image::Rgb([255, 0, 0]));
        img.save(&path).unwrap();

        assert_eq!(apply(&path, &[90]).unwrap(), Outcome::Written);
        let turned = image::open(&path).unwrap().to_rgb8();
        // Clockwise: landscape, and the red is now at the top right.
        assert_eq!(turned.dimensions(), (3, 2));
        assert_eq!(turned.get_pixel(2, 0).0, [255, 0, 0]);

        let other = dir.join("bild.tif");
        std::fs::write(&other, b"egal").unwrap();
        assert_eq!(apply(&other, &[90]).unwrap(), Outcome::Unsupported);
        std::fs::remove_dir_all(&dir).ok();
    }
}
