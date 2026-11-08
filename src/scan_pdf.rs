//! Prepare scanned pages and save them as a searchable PDF: straighten, colour mode (black-and-white document,
//! colour document, brochure, photo), recognise the text with the position of each word and lay it invisibly over
//! the image. Runs in its own thread, which keeps the OCR engine between two scans.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::mpsc::{Receiver, Sender, channel};

use anyhow::{Context, Result, bail};
use eframe::egui;
use image::{DynamicImage, GrayImage, Luma, RgbImage};
use lopdf::content::{Content, Operation};
use lopdf::{Document, Encoding, Object, Stream, StringFormat, dictionary};

use crate::config::{OcrConfig, ScanPreset};
use crate::i18n::scan::t as texts;
use crate::ocr::{Line, Ocr, Word};
use crate::preprocess;
use crate::scan::ScannedPage;

/// Longer side of the image for OCR; more hardly helps but costs time.
const OCR_MAX_SIDE: u32 = 2800;

/// A page image, ready for the PDF.
pub enum Encoded {
    /// 1 bit per pixel (1 = white), each row padded to full bytes.
    Bilevel {
        width: u32,
        height: u32,
        bits: Vec<u8>,
    },
    Jpeg {
        width: u32,
        height: u32,
        data: Vec<u8>,
        gray: bool,
    },
}

impl Encoded {
    fn size(&self) -> (u32, u32) {
        match self {
            Encoded::Bilevel { width, height, .. } | Encoded::Jpeg { width, height, .. } => (*width, *height),
        }
    }
}

/// A page of the PDF: image, resolution and the recognised text with its position in image pixels.
pub struct PdfPage {
    pub image: Encoded,
    pub dpi: f32,
    pub lines: Vec<Line>,
}

/// A scan to prepare.
pub struct Job {
    pub pages: Vec<ScannedPage>,
    pub preset: ScanPreset,
    pub text_layer: bool,
    pub ocr: OcrConfig,
    pub path: PathBuf,
}

pub enum Progress {
    /// What is happening right now and, where known, how far along (0 to 1).
    Step(String, Option<f32>),
    /// Saved; `notes` lists what did not work (e.g. no OCR).
    Saved { path: PathBuf, notes: Vec<String> },
    /// Not saved; the error is shown as a notice, the pages can be scanned again.
    Failed { error: String },
}

struct Request {
    job: Job,
    reply: Sender<Progress>,
    ctx: egui::Context,
}

static WORKER: LazyLock<Sender<Request>> = LazyLock::new(|| {
    let (tx, rx) = channel::<Request>();
    let started = std::thread::Builder::new().name("scan-processing".into()).spawn(move || {
        let mut ocr = None;
        for Request { job, reply, ctx } in rx {
            let send = |p: Progress| {
                let _ = reply.send(p);
                ctx.request_repaint();
            };
            let result = run(job, &mut ocr, &|s: &str, share| send(Progress::Step(s.to_string(), share)));
            send(result);
        }
    });
    // Without a thread the sender has no receiver; `start` then reports `worker_gone`.
    if let Err(e) = started {
        eprintln!("Could not start the scan thread: {e}");
    }
    tx
});

/// Prepares a scan in the background and saves it; progress and result arrive via the receiver.
pub fn start(ctx: &egui::Context, job: Job) -> Receiver<Progress> {
    let (reply, rx) = channel();
    if let Err(std::sync::mpsc::SendError(request)) = WORKER.send(Request { job, reply, ctx: ctx.clone() }) {
        let error = texts().worker_gone.to_string();
        let _ = request.reply.send(Progress::Failed { error });
    }
    rx
}

fn run(job: Job, ocr: &mut Option<Ocr>, step: &dyn Fn(&str, Option<f32>)) -> Progress {
    let t = texts();
    let mut notes = Vec::new();
    // OCR is also needed to tell which way up a page is. Photos stay as they are and get no text: from pictures
    // OCR mostly reads gibberish.
    let wants_ocr = job.preset != ScanPreset::Photo;
    if wants_ocr && ocr.as_ref().is_none_or(|o| o.config() != &job.ocr) {
        *ocr = None;
        step(t.step_ocr, Some(0.0));
        match Ocr::new(&job.ocr, &|s: &str| step(s, None)) {
            Ok(o) => *ocr = Some(o),
            Err(e) => notes.push((t.note_no_ocr)(&format!("{e:#}"))),
        }
    }
    let engine = if wants_ocr { ocr.as_ref() } else { None };
    let n = job.pages.len();
    let mut pages = Vec::with_capacity(n);
    for (i, page) in job.pages.iter().enumerate() {
        // One share per page, saving is the last one.
        step(&(t.step_page)(i + 1, n), Some(i as f32 / (n + 1) as f32));
        let prepared = prepare_page(page, job.preset, engine, job.text_layer, &mut notes).or_else(|e| {
            notes.push((t.note_page_raw)(i + 1, &format!("{e:#}")));
            Ok::<_, anyhow::Error>(PdfPage { image: jpeg(&page.image, 85)?, dpi: page.dpi, lines: Vec::new() })
        });
        match prepared {
            Ok(p) => pages.push(p),
            Err(e) => return Progress::Failed { error: format!("{e:#}") },
        }
    }
    step(t.step_save, Some(n as f32 / (n + 1) as f32));
    match write(&pages, &job.path) {
        Ok(()) => Progress::Saved { path: job.path, notes },
        Err(e) => Progress::Failed { error: format!("{e:#}") },
    }
}

/// Prepares a page: straighten (except for photos), recognise text with positions, apply the colour mode.
pub fn prepare_page(
    page: &ScannedPage,
    preset: ScanPreset,
    ocr: Option<&Ocr>,
    text_layer: bool,
    notes: &mut Vec<String>,
) -> Result<PdfPage> {
    let upright = if preset == ScanPreset::Photo {
        page.image.clone()
    } else {
        preprocess::correct_page(&page.image, false, |probe| ocr.and_then(|o| o.recognize_confident(probe).ok())).image
    };
    let lines = match ocr {
        Some(o) if text_layer && preset != ScanPreset::Photo => {
            let (small, factor) = shrink(&upright, OCR_MAX_SIDE);
            match o.recognize_lines(&preprocess::enhance_for_ocr(&small)) {
                Ok(lines) => scale_lines(lines, 1.0 / factor),
                Err(e) => {
                    notes.push((texts().note_ocr_failed)(&format!("{e:#}")));
                    Vec::new()
                }
            }
        }
        _ => Vec::new(),
    };
    let image = encode(&apply_preset(&upright, preset), preset)?;
    Ok(PdfPage { image, dpi: page.dpi, lines })
}

/// Scales down to at most `max_side` pixels on the longer side; returns the image and the factor.
fn shrink(img: &DynamicImage, max_side: u32) -> (DynamicImage, f32) {
    let longest = img.width().max(img.height());
    if longest <= max_side {
        return (img.clone(), 1.0);
    }
    let factor = max_side as f32 / longest as f32;
    let (w, h) = ((img.width() as f32 * factor).round() as u32, (img.height() as f32 * factor).round() as u32);
    (img.resize_exact(w.max(1), h.max(1), image::imageops::FilterType::Triangle), factor)
}

fn scale_lines(lines: Vec<Line>, f: f32) -> Vec<Line> {
    lines
        .into_iter()
        .map(|line| {
            line.into_iter()
                .map(|w| Word { left: w.left * f, top: w.top * f, right: w.right * f, bottom: w.bottom * f, ..w })
                .collect()
        })
        .collect()
}

/// Applies the colour mode to an image (also for the preview in the dialog after scanning).
pub fn apply_preset(img: &DynamicImage, preset: ScanPreset) -> DynamicImage {
    let keep_gray = |out: RgbImage| {
        if img.color().has_color() {
            DynamicImage::ImageRgb8(out)
        } else {
            DynamicImage::ImageLuma8(DynamicImage::ImageRgb8(out).to_luma8())
        }
    };
    match preset {
        ScanPreset::BlackWhite => DynamicImage::ImageLuma8(binarize(&img.to_luma8())),
        ScanPreset::Color => keep_gray(whiten_paper(img.to_rgb8())),
        ScanPreset::Brochure => keep_gray(vivid(img.to_rgb8())),
        ScanPreset::Photo => img.clone(),
    }
}

fn luma(p: &image::Rgb<u8>) -> u8 {
    ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8
}

/// The value below which the fraction `q` of the pixels lies.
fn percentile(hist: &[u64; 256], q: f64) -> u8 {
    let total: u64 = hist.iter().sum();
    let target = (total as f64 * q) as u64;
    let mut acc = 0;
    for (v, &n) in hist.iter().enumerate() {
        acc += n;
        if acc > target {
            return v as u8;
        }
    }
    255
}

/// Colour document: the paper turns white and the text bold. White is, per channel, the value that 95 % of the
/// pixels do not exceed – for a document, the paper –, black the darkest percent.
fn whiten_paper(mut rgb: RgbImage) -> RgbImage {
    let mut hist = [[0u64; 256]; 3];
    let mut lum = [0u64; 256];
    for p in rgb.pixels() {
        for c in 0..3 {
            hist[c][p[c] as usize] += 1;
        }
        lum[luma(p) as usize] += 1;
    }
    let black = percentile(&lum, 0.01).min(80) as f32;
    let lut: [[u8; 256]; 3] = std::array::from_fn(|c| {
        // At least 160: a dark image without a paper margin does not become glaring.
        let white = (percentile(&hist[c], 0.95) as f32).max(160.0).max(black + 64.0);
        std::array::from_fn(|v| {
            let out = ((v as f32 - black) / (white - black) * 255.0).clamp(0.0, 255.0);
            // Almost white becomes white, so that paper texture and noise disappear.
            if out >= 245.0 { 255 } else { out.round() as u8 }
        })
    });
    for p in rgb.pixels_mut() {
        for c in 0..3 {
            p[c] = lut[c][p[c] as usize];
        }
    }
    rgb
}

/// Brochure: more contrast and slightly stronger colours; the hue stays, the paper does not turn white.
fn vivid(mut rgb: RgbImage) -> RgbImage {
    let mut lum = [0u64; 256];
    for p in rgb.pixels() {
        lum[luma(p) as usize] += 1;
    }
    let lo = percentile(&lum, 0.005) as f32;
    let hi = (percentile(&lum, 0.995) as f32).max(lo + 32.0);
    for p in rgb.pixels_mut() {
        let l = luma(p) as f32;
        for c in 0..3 {
            let saturated = l + (p[c] as f32 - l) * 1.2;
            p[c] = ((saturated - lo) / (hi - lo) * 255.0).clamp(0.0, 255.0).round() as u8;
        }
    }
    rgb
}

/// Black-and-white document: black where a pixel is clearly darker than its surroundings (Bradley's method). Text
/// on a grey background or in shadow stays readable, and blank paper stays white without speckles.
fn binarize(gray: &GrayImage) -> GrayImage {
    let radius = (gray.width().max(gray.height()) / 16).max(2);
    let mean = imageproc::filter::box_filter(gray, radius, radius);
    GrayImage::from_fn(gray.width(), gray.height(), |x, y| {
        let p = gray.get_pixel(x, y)[0] as u32;
        let m = mean.get_pixel(x, y)[0] as u32;
        let black = p < 60 || (p < 230 && p * 100 < m * 85);
        Luma([if black { 0 } else { 255 }])
    })
}

/// The image encoded by colour mode: black-and-white with 1 bit, otherwise JPEG – for photos in higher quality.
fn encode(img: &DynamicImage, preset: ScanPreset) -> Result<Encoded> {
    match preset {
        ScanPreset::BlackWhite => {
            let gray = img.to_luma8();
            Ok(Encoded::Bilevel { width: gray.width(), height: gray.height(), bits: pack_bits(&gray) })
        }
        ScanPreset::Color => jpeg(img, 80),
        ScanPreset::Brochure => jpeg(img, 85),
        ScanPreset::Photo => jpeg(img, 92),
    }
}

/// 1 bit per pixel, most significant bit on the left, rows padded to full bytes; 1 = white as in DeviceGray.
fn pack_bits(gray: &GrayImage) -> Vec<u8> {
    let stride = gray.width().div_ceil(8) as usize;
    let mut out = vec![0xFFu8; stride * gray.height() as usize];
    for (x, y, p) in gray.enumerate_pixels() {
        if p[0] < 128 {
            out[y as usize * stride + x as usize / 8] &= !(0x80 >> (x % 8));
        }
    }
    out
}

/// Greyscale JPEG if the image has no colour; otherwise RGB.
pub fn jpeg(image: &DynamicImage, quality: u8) -> Result<Encoded> {
    let gray = !image.color().has_color();
    let mut data = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut data, quality);
    if gray {
        encoder.encode_image(&image.to_luma8())?;
    } else {
        encoder.encode_image(&image.to_rgb8())?;
    }
    Ok(Encoded::Jpeg { width: image.width(), height: image.height(), data, gray })
}

/// Writes the pages as a PDF in paper size (from pixels and resolution). Recognised text lies invisibly (rendering
/// mode 3) over the image, word by word in its place: searching, selecting and copying work.
pub fn write(pages: &[PdfPage], path: &Path) -> Result<()> {
    if pages.is_empty() {
        bail!(texts().no_pages);
    }
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let mut kids = Vec::new();
    for page in pages {
        let (w_px, h_px) = page.image.size();
        let scale = 72.0 / page.dpi;
        let (w, h) = (w_px as f32 * scale, h_px as f32 * scale);
        let image = match &page.image {
            Encoded::Bilevel { bits, .. } => {
                let mut s = Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Image",
                        "Width" => w_px,
                        "Height" => h_px,
                        "ColorSpace" => "DeviceGray",
                        "BitsPerComponent" => 1,
                    },
                    bits.clone(),
                );
                s.compress()?;
                s
            }
            Encoded::Jpeg { data, gray, .. } => Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => w_px,
                    "Height" => h_px,
                    "ColorSpace" => if *gray { "DeviceGray" } else { "DeviceRGB" },
                    "BitsPerComponent" => 8,
                    "Filter" => "DCTDecode",
                },
                data.clone(),
            ),
        };
        let image_id = doc.add_object(image);
        let mut operations = vec![
            Operation::new("q", vec![]),
            Operation::new("cm", vec![w.into(), 0.into(), 0.into(), h.into(), 0.into(), 0.into()]),
            Operation::new("Do", vec!["Scan".into()]),
            Operation::new("Q", vec![]),
        ];
        operations.extend(text_layer(&page.lines, scale, h));
        let mut content = Stream::new(dictionary! {}, Content { operations }.encode()?);
        content.compress()?;
        let content_id = doc.add_object(content);
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), w.into(), h.into()],
            "Contents" => content_id,
            "Resources" => dictionary! {
                "XObject" => dictionary! { "Scan" => image_id },
                "Font" => dictionary! { "F1" => font_id },
            },
        });
        kids.push(Object::from(page_id));
    }
    let count = kids.len() as i64;
    doc.objects
        .insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut file = std::fs::File::create(path).with_context(|| (texts().cannot_create)(&path.display().to_string()))?;
    doc.save_to(&mut file)?;
    Ok(())
}

/// Invisible text: per line a font size from its height, per word a text origin at its left edge and a horizontal
/// scaling to its width. A space separates the words of a line.
fn text_layer(lines: &[Line], scale: f32, page_height: f32) -> Vec<Operation> {
    let encoding = Encoding::SimpleEncoding(b"WinAnsiEncoding");
    let mut ops = Vec::new();
    for line in lines {
        let top = line.iter().map(|w| w.top).fold(f32::MAX, f32::min);
        let bottom = line.iter().map(|w| w.bottom).fold(f32::MIN, f32::max);
        let size = ((bottom - top) * scale).max(1.0);
        // The baseline sits about one descender above the bottom edge of the line.
        let baseline = page_height - bottom * scale + size * 0.2;
        let mut first = true;
        for (i, word) in line.iter().enumerate() {
            let mut bytes = encoding.string_to_bytes(&word.text);
            let units: u32 = bytes.iter().map(|&b| helvetica_width(b) as u32).sum();
            if units == 0 {
                continue;
            }
            if first {
                ops.push(Operation::new("Tf", vec!["F1".into(), size.into()]));
                first = false;
            }
            let natural = units as f32 / 1000.0 * size;
            let stretch = ((word.right - word.left) * scale).max(0.5) / natural;
            if i + 1 < line.len() {
                bytes.push(b' ');
            }
            ops.push(Operation::new(
                "Tm",
                vec![stretch.into(), 0.into(), 0.into(), 1.into(), (word.left * scale).into(), baseline.into()],
            ));
            ops.push(Operation::new("Tj", vec![Object::String(bytes, StringFormat::Hexadecimal)]));
        }
    }
    if ops.is_empty() {
        return ops;
    }
    let mut all = vec![Operation::new("BT", vec![]), Operation::new("Tr", vec![3.into()])];
    all.extend(ops);
    all.push(Operation::new("ET", vec![]));
    all
}

/// Character widths of Helvetica in WinAnsi (thousandths of the font size), as every PDF reader knows them.
fn helvetica_width(b: u8) -> u16 {
    match b {
        b' ' | b'!' | b',' | b'.' | b'/' | b':' | b';' | b'I' | b'[' | b'\\' | b']' | b'f' | b't' => 278,
        b'"' => 355,
        b'#' | b'$' | b'0'..=b'9' | b'?' | b'L' | b'_' | b'a' | b'b' | b'd' | b'e' | b'g' | b'h' | b'n' | b'o' => 556,
        b'p' | b'q' | b'u' => 556,
        b'%' => 889,
        b'&' | b'A' | b'B' | b'E' | b'K' | b'P' | b'S' | b'V' | b'X' | b'Y' => 667,
        b'\'' => 191,
        b'(' | b')' | b'-' | b'`' | b'r' => 333,
        b'*' => 389,
        b'+' | b'<' | b'=' | b'>' | b'~' => 584,
        b'@' => 1015,
        b'C' | b'D' | b'H' | b'N' | b'R' | b'U' | b'w' => 722,
        b'F' | b'T' | b'Z' => 611,
        b'G' | b'O' | b'Q' => 778,
        b'J' | b'c' | b'k' | b's' | b'v' | b'x' | b'y' | b'z' => 500,
        b'M' | b'm' => 833,
        b'W' => 944,
        b'^' => 469,
        b'i' | b'j' | b'l' => 222,
        b'{' | b'}' => 334,
        b'|' => 260,
        // € ‚ „ … ‰ Œ ‘ ’ “ ” • – — ™ and the accented letters
        0x82 | 0x91 | 0x92 => 222,
        0x84 | 0x8B | 0x93 | 0x94 | 0x9B => 333,
        0x85 | 0x89 | 0x8C | 0x97 | 0x99 | 0xC6 => 1000,
        0x95 => 350,
        0xC0..=0xC5 | 0xC8..=0xCB | 0xDD | 0xDE => 667,
        0xC7 | 0xD0 | 0xD1 | 0xD9..=0xDC => 722,
        0xCC..=0xCF | 0xEC..=0xEF => 278,
        0xD2..=0xD6 | 0xD8 => 778,
        0xD7 | 0xF7 => 584,
        0xDF | 0xF8 => 611,
        0xE6 => 889,
        0xE7 | 0xFD | 0xFF => 500,
        _ => 556,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, left: f32, top: f32, right: f32, bottom: f32) -> Word {
        Word { text: text.into(), left, top, right, bottom }
    }

    #[test]
    fn bits_are_packed_msb_first_with_padding() {
        // 10 × 2: top row pixels 0 and 9 black, bottom row all white.
        let mut img = GrayImage::from_pixel(10, 2, Luma([255]));
        img.put_pixel(0, 0, Luma([0]));
        img.put_pixel(9, 0, Luma([0]));
        assert_eq!(pack_bits(&img), vec![0b0111_1111, 0b1011_1111, 0xFF, 0xFF]);
    }

    #[test]
    fn umlauts_and_euro_are_win_ansi() {
        let bytes = Encoding::SimpleEncoding(b"WinAnsiEncoding").string_to_bytes("Straße 5 €");
        assert_eq!(bytes, b"Stra\xdfe 5 \x80");
        assert_eq!(helvetica_width(b'm'), 833);
        assert_eq!(helvetica_width(0xFC), 556); // ü
    }

    #[test]
    fn black_white_keeps_text_and_clears_paper() {
        // Light grey paper with a slight gradient and a dark bar as "text".
        let gray = GrayImage::from_fn(200, 100, |x, y| {
            if (40..160).contains(&x) && (45..55).contains(&y) { Luma([70]) } else { Luma([200 + (x % 7) as u8]) }
        });
        let bw = binarize(&gray);
        assert_eq!(bw.get_pixel(100, 50)[0], 0);
        assert!(bw.pixels().filter(|p| p[0] == 0).count() <= 120 * 10);
        assert_eq!(bw.get_pixel(10, 10)[0], 255);
    }

    #[test]
    fn colour_documents_get_white_paper() {
        let rgb =
            RgbImage::from_fn(
                100,
                100,
                |x, _| if x < 10 { image::Rgb([20, 20, 60]) } else { image::Rgb([225, 215, 190]) },
            );
        let out = whiten_paper(rgb);
        assert_eq!(out.get_pixel(50, 50).0, [255, 255, 255]);
        assert!(out.get_pixel(5, 5)[0] < 30);
    }

    #[test]
    fn pages_get_their_paper_size() {
        let dir = std::env::temp_dir().join(format!("scrinium-paper-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("scan.pdf");
        let color = DynamicImage::ImageRgb8(RgbImage::from_pixel(827, 1169, image::Rgb([250, 240, 230])));
        let gray = DynamicImage::ImageLuma8(GrayImage::from_pixel(827, 1169, Luma([200])));
        // A4 at 100 dpi: 827 × 1169 pixels give 595 × 842 points.
        let page = |img: &DynamicImage| PdfPage { image: jpeg(img, 85).unwrap(), dpi: 100.0, lines: Vec::new() };
        write(&[page(&color), page(&gray)], &path).unwrap();
        let doc = Document::load(&path).unwrap();
        let pages = doc.get_pages();
        assert_eq!(pages.len(), 2);
        let first = doc.get_dictionary(pages[&1]).unwrap();
        let mbox = first.get(b"MediaBox").unwrap().as_array().unwrap();
        let w = mbox[2].as_float().unwrap();
        let h = mbox[3].as_float().unwrap();
        assert!((w - 595.4).abs() < 1.0 && (h - 841.7).abs() < 1.0, "{w} × {h}");
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(write(&[], &path).is_err());
    }

    #[test]
    fn searchable_pdf_carries_the_text() {
        let dir = std::env::temp_dir().join(format!("scrinium-scanpdf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("scan.pdf");
        let image = DynamicImage::ImageLuma8(GrayImage::from_pixel(827, 1169, Luma([255])));
        let lines = vec![
            vec![word("Grundsteuerbescheid", 100.0, 100.0, 520.0, 130.0), word("2026", 540.0, 100.0, 640.0, 130.0)],
            vec![word("Müller", 100.0, 150.0, 240.0, 180.0)],
        ];
        let pages = [
            PdfPage { image: encode(&image, ScanPreset::BlackWhite).unwrap(), dpi: 100.0, lines },
            PdfPage { image: jpeg(&image, 80).unwrap(), dpi: 100.0, lines: Vec::new() },
        ];
        write(&pages, &path).unwrap();
        let doc = Document::load(&path).unwrap();
        assert_eq!(doc.get_pages().len(), 2);
        let text = doc.extract_text(&[1]).unwrap();
        assert!(text.contains("Grundsteuerbescheid 2026"), "{text:?}");
        assert!(text.contains("Müller"), "{text:?}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
