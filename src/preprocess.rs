//! Image preparation for pages scanned skewed or rotated:
//! coarse rotation (0/90/180/270°), deskewing and contrast enhancement.

use image::{DynamicImage, GrayImage, Rgb, RgbImage, imageops};
use imageproc::geometric_transformations::{Interpolation, rotate_about_center};

/// Side length the image is scaled down to for the angle estimate.
const ANALYSIS_MAX_SIDE: u32 = 1000;
const MAX_SKEW_DEG: f32 = 15.0;
/// Skew below this value is not corrected.
const MIN_SKEW_DEG: f32 = 0.15;
/// Minimum readable text (see `text_plausibility`) a sideways rotation needs, about three common words.
const MIN_SIDEWAYS_SCORE: f32 = 8.0;

pub struct CorrectedPage {
    pub image: DynamicImage,
    /// Coarse rotation applied clockwise, in degrees.
    pub rotation: u16,
    /// Corrected skew in degrees.
    pub skew: f32,
}

/// Straightens a page. `probe` returns the recognised text for a greyscale image and so decides between 0° and
/// 180° (or 90° and 270°). With `sideways` it checks 90° and 270° even if the line profile does not consider the
/// page sideways (an outside reason, e.g. the text layer).
pub fn correct_page<F>(img: &DynamicImage, sideways: bool, mut probe: F) -> CorrectedPage
where
    F: FnMut(&GrayImage) -> Option<String>,
{
    let small = downscale_gray(img, ANALYSIS_MAX_SIDE);
    let mut best_of = |candidates: [u16; 2]| {
        let mut best = (candidates[0], f32::MIN);
        for rot in candidates {
            let probe_img = probe_region(&rotate_gray(&small, rot));
            let score = probe(&probe_img).map(|t| text_plausibility(&t)).unwrap_or(0.0);
            if score > best.1 {
                best = (rot, score);
            }
        }
        best
    };
    let upright = best_of([0, 180]);
    // The line profile sometimes mistakes large photos and columns for sideways text. So a page counts as
    // sideways only if OCR finds clearly more readable text sideways than upright – and a few words at all
    // (a photo with little text stays as it is).
    let rotation = if sideways || is_sideways(&small) {
        let side = best_of([90, 270]);
        if side.1 >= MIN_SIDEWAYS_SCORE && side.1 > upright.1 * 1.25 + 2.0 { side.0 } else { upright.0 }
    } else {
        upright.0
    };

    let rotated = rotate_dynamic(img, rotation);
    let skew = estimate_skew(&downscale_gray(&rotated, ANALYSIS_MAX_SIDE));
    let image = if skew.abs() >= MIN_SKEW_DEG { deskew(&rotated, skew) } else { rotated };
    CorrectedPage { image, rotation, skew }
}

/// Is the page's text sideways? Quick pre-check via the line profile, without OCR.
pub fn looks_sideways(img: &DynamicImage) -> bool {
    is_sideways(&downscale_gray(img, ANALYSIS_MAX_SIDE))
}

/// Greyscale with stretched contrast, as input for OCR.
pub fn enhance_for_ocr(img: &DynamicImage) -> GrayImage {
    let mut gray = img.to_luma8();
    let mut hist = [0u64; 256];
    for p in gray.pixels() {
        hist[p.0[0] as usize] += 1;
    }
    let total = gray.width() as u64 * gray.height() as u64;
    let percentile = |q: f64| -> u8 {
        let target = (total as f64 * q) as u64;
        let mut acc = 0;
        for (v, &n) in hist.iter().enumerate() {
            acc += n;
            if acc >= target {
                return v as u8;
            }
        }
        255
    };
    let lo = percentile(0.01) as f32;
    let hi = percentile(0.99) as f32;
    if hi - lo > 10.0 && (lo > 5.0 || hi < 250.0) {
        let scale = 255.0 / (hi - lo);
        for p in gray.pixels_mut() {
            p.0[0] = ((p.0[0] as f32 - lo) * scale).clamp(0.0, 255.0) as u8;
        }
    }
    gray
}

pub fn downscale_gray(img: &DynamicImage, max_side: u32) -> GrayImage {
    let (w, h) = (img.width(), img.height());
    let longest = w.max(h);
    if longest <= max_side {
        return img.to_luma8();
    }
    let f = max_side as f32 / longest as f32;
    let nw = ((w as f32 * f).round() as u32).max(1);
    let nh = ((h as f32 * f).round() as u32).max(1);
    imageops::resize(&img.to_luma8(), nw, nh, imageops::FilterType::Triangle)
}

pub fn rotate_dynamic(img: &DynamicImage, deg: u16) -> DynamicImage {
    match deg % 360 {
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => img.clone(),
    }
}

fn rotate_gray(img: &GrayImage, deg: u16) -> GrayImage {
    match deg % 360 {
        90 => imageops::rotate90(img),
        180 => imageops::rotate180(img),
        270 => imageops::rotate270(img),
        _ => img.clone(),
    }
}

/// Middle strip of the page, so the probe OCR stays fast. It is centred so that 0° and 180° (and 90° and 270°)
/// read the same spot: a photo at the top and body text at the bottom would otherwise favour the rotated page.
fn probe_region(img: &GrayImage) -> GrayImage {
    let h = ((img.height() as f32 * 0.55).round() as u32).max(1);
    imageops::crop_imm(img, 0, (img.height() - h) / 2, img.width(), h).to_image()
}

/// Coordinates of dark pixels (Otsu threshold), thinned out if needed.
fn dark_pixels(img: &GrayImage) -> Vec<(f32, f32)> {
    let level = imageproc::contrast::otsu_level(img);
    let mut pts = Vec::new();
    for (x, y, p) in img.enumerate_pixels() {
        if p.0[0] < level {
            pts.push((x as f32, y as f32));
        }
    }
    const MAX_POINTS: usize = 150_000;
    if pts.len() > MAX_POINTS {
        let step = pts.len().div_ceil(MAX_POINTS);
        pts = pts.into_iter().step_by(step).collect();
    }
    pts
}

/// Rates how "line-like" the dark pixels are after rotating by `angle_rad`.
/// Same mapping as `rotate_about_center`: y' = x·sin θ + y·cos θ.
fn projection_score(pts: &[(f32, f32)], cx: f32, cy: f32, angle_rad: f32, bins: usize) -> f64 {
    let (s, c) = angle_rad.sin_cos();
    let half = bins as f32 / 2.0;
    let mut hist = vec![0u32; bins];
    for &(x, y) in pts {
        let yr = (x - cx) * s + (y - cy) * c + half;
        if yr >= 0.0 && (yr as usize) < bins {
            hist[yr as usize] += 1;
        }
    }
    hist.windows(2).map(|w| (w[1] as f64 - w[0] as f64).powi(2)).sum()
}

/// Returns the angle in degrees by which `deskew` must rotate so that text lines lie horizontal.
pub fn estimate_skew(img: &GrayImage) -> f32 {
    let pts = dark_pixels(img);
    if pts.len() < 200 {
        return 0.0;
    }
    let (cx, cy) = (img.width() as f32 / 2.0, img.height() as f32 / 2.0);
    let bins = ((img.width().pow(2) + img.height().pow(2)) as f32).sqrt() as usize + 2;
    let score = |deg: f32| projection_score(&pts, cx, cy, deg.to_radians(), bins);

    let search = |from: f32, to: f32, step: f32| -> f32 {
        let mut best = (0.0f32, f64::MIN);
        let mut a = from;
        while a <= to + 1e-4 {
            let s = score(a);
            if s > best.1 {
                best = (a, s);
            }
            a += step;
        }
        best.0
    };
    let coarse = search(-MAX_SKEW_DEG, MAX_SKEW_DEG, 1.0);
    let fine = search(coarse - 1.0, coarse + 1.0, 0.1);
    (fine * 10.0).round() / 10.0
}

/// Detects whether text lines run vertically (page rotated by 90°).
fn is_sideways(img: &GrayImage) -> bool {
    let pts = dark_pixels(img);
    if pts.len() < 200 {
        return false;
    }
    let (cx, cy) = (img.width() as f32 / 2.0, img.height() as f32 / 2.0);
    let bins = ((img.width().pow(2) + img.height().pow(2)) as f32).sqrt() as usize + 2;
    let best_near = |center: f32| -> f64 {
        let mut best = f64::MIN;
        let mut a = center - MAX_SKEW_DEG;
        while a <= center + MAX_SKEW_DEG {
            best = best.max(projection_score(&pts, cx, cy, a.to_radians(), bins));
            a += 1.5;
        }
        best
    };
    best_near(90.0) > best_near(0.0) * 1.2
}

/// Rotates by `deg` degrees on an enlarged white canvas so that no corners are cut off.
pub fn deskew(img: &DynamicImage, deg: f32) -> DynamicImage {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let rad = deg.to_radians();
    let (s, c) = (rad.sin().abs(), rad.cos().abs());
    let nw = (w as f32 * c + h as f32 * s).ceil() as u32;
    let nh = (w as f32 * s + h as f32 * c).ceil() as u32;
    let white = Rgb([255u8, 255, 255]);
    let mut canvas = RgbImage::from_pixel(nw, nh, white);
    imageops::overlay(&mut canvas, &rgb, ((nw - w) / 2) as i64, ((nh - h) / 2) as i64);
    DynamicImage::ImageRgb8(rotate_about_center(&canvas, rad, Interpolation::Bilinear, white))
}

const COMMON_WORDS: &[&str] = &[
    "der",
    "die",
    "das",
    "und",
    "ist",
    "mit",
    "von",
    "den",
    "dem",
    "des",
    "für",
    "fur",
    "sie",
    "ihre",
    "ihr",
    "wir",
    "uns",
    "auf",
    "bei",
    "zum",
    "zur",
    "nicht",
    "eine",
    "einen",
    "datum",
    "rechnung",
    "seite",
    "betrag",
    "herr",
    "frau",
    "straße",
    "strasse",
    "gmbh",
    "nr",
    "tel",
    "the",
    "and",
    "for",
    "you",
    "your",
    "sehr",
    "geehrte",
    "geehrter",
    "freundlichen",
    "grüßen",
    "grussen",
    "vom",
    "bis",
    "euro",
    "eur",
];

/// Heuristic score of how much OCR output looks like real text.
/// Upside-down text yields mostly punctuation and jumbled letters.
pub fn text_plausibility(text: &str) -> f32 {
    let mut score = 0.0;
    for raw in text.split_whitespace() {
        let word: String = raw.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        if word.chars().count() < 2 {
            continue;
        }
        let letters = word.chars().filter(|c| c.is_alphabetic()).count();
        let digits = word.chars().filter(|c| c.is_ascii_digit()).count();
        let total = word.chars().count();
        if COMMON_WORDS.contains(&word.as_str()) {
            score += 3.0;
        } else if letters == total && word.chars().any(|c| "aeiouäöüy".contains(c)) {
            score += 1.0;
        } else if digits == total {
            score += 0.5;
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;

    /// Creates a pseudo text page: lines of dark "word" blocks.
    fn synthetic_page() -> DynamicImage {
        let mut img = RgbImage::from_pixel(800, 1100, Rgb([255, 255, 255]));
        let mut seed = 7u32;
        for line in 0..30 {
            let y0 = 80 + line * 32;
            let mut x = 70;
            while x < 720 {
                seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                let wlen = 25 + (seed >> 16) % 70;
                for yy in y0..y0 + 14 {
                    for xx in x..(x + wlen).min(730) {
                        img.put_pixel(xx, yy, Rgb([20, 20, 20]));
                    }
                }
                x += wlen + 14;
            }
        }
        DynamicImage::ImageRgb8(img)
    }

    fn rotate_by(img: &DynamicImage, deg: f32) -> DynamicImage {
        deskew(img, deg)
    }

    #[test]
    fn straight_page_has_no_skew() {
        let page = synthetic_page();
        let skew = estimate_skew(&downscale_gray(&page, ANALYSIS_MAX_SIDE));
        assert!(skew.abs() < 0.3, "skew {skew}");
    }

    #[test]
    fn detects_and_corrects_skew() {
        for tilt in [-7.0f32, -2.5, 4.0, 11.0] {
            let tilted = rotate_by(&synthetic_page(), tilt);
            let est = estimate_skew(&downscale_gray(&tilted, ANALYSIS_MAX_SIDE));
            assert!((est + tilt).abs() < 0.5, "tilt {tilt}, estimate {est}");
            let fixed = deskew(&tilted, est);
            let rest = estimate_skew(&downscale_gray(&fixed, ANALYSIS_MAX_SIDE));
            assert!(rest.abs() < 0.5, "tilt {tilt}, residual {rest}");
        }
    }

    #[test]
    fn detects_sideways_pages() {
        let page = synthetic_page();
        assert!(!is_sideways(&downscale_gray(&page, ANALYSIS_MAX_SIDE)));
        assert!(is_sideways(&downscale_gray(&page.rotate90(), ANALYSIS_MAX_SIDE)));
    }

    /// Straightens `page`; the probe reads text only on the `readable`-th call. Order of the probes:
    /// 0°, 180°, then 90° and 270° for pages that look sideways.
    fn corrected_with_readable_probe(page: &DynamicImage, readable: usize) -> CorrectedPage {
        let mut calls = 0;
        correct_page(page, false, |_img| {
            calls += 1;
            if calls == readable {
                Some("Sehr geehrte Frau Müller, anbei die Rechnung für den Monat".into())
            } else {
                Some("'; ,.! ~".into())
            }
        })
    }

    #[test]
    fn probe_picks_readable_orientation() {
        let sideways = synthetic_page().rotate90();
        let corrected = corrected_with_readable_probe(&sideways, 4);
        assert_eq!(corrected.rotation, 270);
        assert_eq!(corrected.image.width(), 800);
        let upside_down = synthetic_page().rotate180();
        assert_eq!(corrected_with_readable_probe(&upside_down, 2).rotation, 180);
    }

    #[test]
    fn readable_upright_text_beats_a_sideways_profile() {
        // The profile considers the page sideways (as with magazine pages with large photos), but upright it
        // reads as text: the page stays as it is.
        let page = synthetic_page().rotate90();
        assert!(is_sideways(&downscale_gray(&page, ANALYSIS_MAX_SIDE)));
        let corrected = corrected_with_readable_probe(&page, 1);
        assert_eq!(corrected.rotation, 0);
        assert_eq!(corrected.image.width(), page.width());
    }

    #[test]
    fn plausibility_prefers_real_text() {
        assert!(
            text_plausibility("Sehr geehrte Damen und Herren, anbei die Rechnung")
                > text_plausibility("ʇ ,: ·l1 'Iq ;; pɹ ~")
        );
    }

    #[test]
    fn enhance_stretches_contrast() {
        let img = DynamicImage::ImageLuma8(GrayImage::from_fn(100, 10, |x, _| Luma([100 + (x as u8) / 2])));
        let out = enhance_for_ocr(&img);
        let max = out.pixels().map(|p| p.0[0]).max().unwrap();
        let min = out.pixels().map(|p| p.0[0]).min().unwrap();
        assert!(max > 240 && min < 15);
    }
}
