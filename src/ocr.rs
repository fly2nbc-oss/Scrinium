//! OCR with Tesseract (`tesseract.rs`): load and verify language data, recognise text and word positions.
//!
//! The language data come from `tessdata_fast` or `tessdata_best` (Apache-2.0) at a fixed commit. The five UI
//! languages are verified with SHA-256, others only by their size and by Tesseract loading them. On Linux,
//! Scrinium uses the system's language data if all selected languages are there (packages tesseract-ocr-deu …)
//! and "fast" is selected; otherwise, as on Windows, its own cache.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::{Result, bail};
use image::GrayImage;
use sha2::{Digest, Sha256};

use crate::config::{self, OcrConfig, OcrQuality};
use crate::download;
use crate::i18n::analysis as texts;
use crate::tesseract::{self, Engine, InitError};

/// Commits of `tessdata_fast` and `tessdata_best` from which the language data are downloaded.
const FAST_COMMIT: &str = "87416418657359cb625c412a48b6e1d6d41c29bd";
const BEST_COMMIT: &str = "e12c65a915945e4c28e237a9b52bc4a8f39a0cec";

/// Language data of the UI languages: code, SHA-256 fast, SHA-256 best.
const CHECKSUMS: [(&str, &str, &str); 5] = [
    (
        "deu",
        "19d219bbb6672c869d20a9636c6816a81eb9a71796cb93ebe0cb1530e2cdb22d",
        "8407331d6aa0229dc927685c01a7938fc5a641d1a9524f74838cdac599f0d06e",
    ),
    (
        "eng",
        "7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2",
        "8280aed0782fe27257a68ea10fe7ef324ca0f8d85bd2fd145d1c2b560bcb66ba",
    ),
    (
        "fra",
        "ced037562e8c80c13122dece28dd477d399af80911a28791a66a63ac1e3445ca",
        "907743d98915c91a3906dfbf6e48b97598346698fe53aaa797e1a064ffcac913",
    ),
    (
        "spa",
        "6f2e04d02774a18f01bed44b1111f2cd7f3ba7ac9dc4373cd3f898a40ea6b464",
        "e2c1ffdad8b30f26c45d4017a9183d3a7f9aa69e59918be4f88b126fac99ab2c",
    ),
    (
        "ita",
        "b8f89e1e785118dac4d51ae042c029a64edb5c3ee42ef73027a6d412748d8827",
        "8df9c89176fb93f56bf4b2d4ede04c01c1f31d4b7697fbd76cc336df700f3f38",
    ),
];

/// No language file is smaller; anything smaller that arrives is an error page.
const MIN_TRAINEDDATA: u64 = 100_000;

pub struct Ocr {
    engine: Engine,
    config: OcrConfig,
    name: String,
}

impl Ocr {
    /// Starts Tesseract with the languages from `config`. Missing language data are downloaded, corrupt ones once
    /// more; `status` reports the download.
    pub fn new(config: &OcrConfig, status: &dyn Fn(&str)) -> Result<Self> {
        let t = texts::t();
        let languages = config.language_list();
        if languages.is_empty() {
            bail!(t.no_languages);
        }
        let joined = languages.join("+");
        let mut retried = false;
        let engine = loop {
            let (dir, own) = tessdata(config, &languages, status)?;
            match Engine::new(&dir, &joined) {
                Ok(engine) => break engine,
                Err(InitError::Other(e)) => return Err(e),
                Err(InitError::Missing(missing)) => {
                    if own {
                        for code in &missing {
                            std::fs::remove_file(dir.join(format!("{code}.traineddata"))).ok();
                        }
                    }
                    if !own || retried {
                        bail!((t.tesseract_languages_missing)(&missing.join("+")));
                    }
                    retried = true;
                }
            }
        };
        let name = (t.ocr_name)(&tesseract::version()?, &joined);
        Ok(Self { engine, config: config.clone(), name })
    }

    pub fn config(&self) -> &OcrConfig {
        &self.config
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn recognize(&self, img: &GrayImage) -> Result<String> {
        self.engine.text(img, assumed_dpi(img))
    }

    /// Text with the position of each word, line by line in reading order – for the text layer of a PDF.
    pub fn recognize_lines(&self, img: &GrayImage) -> Result<Vec<Line>> {
        Ok(parse_tsv(&self.engine.tsv(img, assumed_dpi(img))?))
    }

    /// For comparing the orientations of a page: only horizontal lines (see `Engine::tsv_single_block`) and only
    /// the words Tesseract is confident about. In the wrong orientation Tesseract yields a lot of letter-like
    /// garbage, but with low confidence.
    pub fn recognize_confident(&self, img: &GrayImage) -> Result<String> {
        Ok(confident_words(&self.engine.tsv_single_block(img, assumed_dpi(img))?))
    }
}

/// From this confidence (0–100) on, a word counts in the orientation probe.
const MIN_PROBE_CONFIDENCE: f32 = 70.0;

fn confident_words(tsv: &str) -> String {
    let words = tsv.lines().filter_map(|row| {
        let cols: Vec<&str> = row.split('\t').collect();
        let confident = cols.len() >= 12
            && cols[0] == "5"
            && cols[10].trim().parse::<f32>().unwrap_or(-1.0) >= MIN_PROBE_CONFIDENCE;
        confident.then(|| cols[11].trim()).filter(|w| !w.is_empty())
    });
    words.collect::<Vec<_>>().join(" ")
}

/// Resolution when nothing else is known: as if the longer side were 29.7 cm long (DIN A4).
fn assumed_dpi(img: &GrayImage) -> u32 {
    (img.width().max(img.height()) as f32 / (297.0 / 25.4)).round() as u32
}

/// The folder with all selected languages; `true` if it is Scrinium's own (then it may delete files).
fn tessdata(config: &OcrConfig, languages: &[String], status: &dyn Fn(&str)) -> Result<(PathBuf, bool)> {
    #[cfg(target_os = "linux")]
    if config.quality == OcrQuality::Fast
        && let Some(dir) = system_tessdata(languages)
    {
        return Ok((dir, false));
    }
    let dir = own_tessdata(config.quality)?;
    for code in languages {
        ensure_language(&dir, code, config.quality, status)?;
    }
    Ok((dir, true))
}

/// The language data of the Linux packages (tesseract-ocr-deu …), if all selected languages are there.
#[cfg(target_os = "linux")]
fn system_tessdata(languages: &[String]) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(prefix) = std::env::var_os("TESSDATA_PREFIX").map(PathBuf::from) {
        dirs.push(prefix.join("tessdata"));
        dirs.push(prefix);
    }
    for d in ["/usr/share/tesseract-ocr/5/tessdata", "/usr/share/tesseract-ocr/4.00/tessdata", "/usr/share/tessdata"] {
        dirs.push(PathBuf::from(d));
    }
    dirs.into_iter().find(|d| languages.iter().all(|l| d.join(format!("{l}.traineddata")).is_file()))
}

/// `<Cache>/tessdata/fast` or `best` (or under `SCRINIUM_TESSDATA`). If Tesseract cannot read the path
/// (characters outside the Windows code page in the user name), Scrinium falls back to
/// `%ProgramData%\Scrinium\tessdata`.
fn own_tessdata(quality: OcrQuality) -> Result<PathBuf> {
    let sub = match quality {
        OcrQuality::Fast => "fast",
        OcrQuality::Best => "best",
    };
    let base = std::env::var_os("SCRINIUM_TESSDATA")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| config::cache_dir().join("tessdata"));
    let dir = base.join(sub);
    if tesseract::c_path(&dir).is_ok() {
        return Ok(dir);
    }
    match std::env::var_os("ProgramData").map(|p| PathBuf::from(p).join("Scrinium").join("tessdata").join(sub)) {
        Some(fallback) if tesseract::c_path(&fallback).is_ok() => Ok(fallback),
        _ => bail!((texts::t().tessdata_path_unusable)(&dir.display().to_string())),
    }
}

/// Downloads the language data `code` into `dir` if they are missing there, and verifies them.
fn ensure_language(dir: &Path, code: &str, quality: OcrQuality, status: &dyn Fn(&str)) -> Result<()> {
    let t = texts::t();
    let file = dir.join(format!("{code}.traineddata"));
    if file.is_file() {
        return Ok(());
    }
    // Codes like "deu", "chi_sim" or "deu_latf"; anything else does not exist and does not belong in a URL.
    if code.is_empty() || !code.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
        bail!((t.language_unknown)(code));
    }
    let (repo, commit) = match quality {
        OcrQuality::Fast => ("tessdata_fast", FAST_COMMIT),
        OcrQuality::Best => ("tessdata_best", BEST_COMMIT),
    };
    let url = format!("https://raw.githubusercontent.com/tesseract-ocr/{repo}/{commit}/{code}.traineddata");
    let expected = CHECKSUMS.iter().find(|(c, ..)| *c == code).map(|(_, fast, best)| match quality {
        OcrQuality::Fast => *fast,
        OcrQuality::Best => *best,
    });
    let announced = Cell::new(false);
    let cancel = AtomicBool::new(false);
    let loaded = download::download_file(
        &url,
        &file,
        code,
        &mut |p| {
            if let Some(total) = p.total.filter(|_| !announced.get()) {
                announced.set(true);
                status(&(t.loading_language)(code, &download::format_bytes(total)));
            }
        },
        &cancel,
    );
    if let Err(e) = loaded {
        // Unknown codes end in 404: say that instead of the URL.
        return Err(if expected.is_none() { anyhow::anyhow!((t.language_unknown)(code)) } else { e });
    }
    let bytes = std::fs::read(&file)?;
    let valid = match expected {
        Some(sum) => sha256_hex(&bytes) == sum,
        None => bytes.len() as u64 >= MIN_TRAINEDDATA,
    };
    if !valid {
        std::fs::remove_file(&file).ok();
        bail!((t.language_corrupt)(code));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A recognised word with its rectangle in image pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// The words of a line, from left to right.
pub type Line = Vec<Word>;

/// Reads Tesseract's TSV output (with or without header row): words (level 5), grouped into lines by block,
/// paragraph and line.
fn parse_tsv(tsv: &str) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut current = None;
    for row in tsv.lines() {
        let cols: Vec<&str> = row.split('\t').collect();
        if cols.len() < 12 || cols[0] != "5" {
            continue;
        }
        let text = cols[11].trim();
        let num = |i: usize| cols[i].trim().parse::<f32>().unwrap_or(0.0);
        if text.is_empty() || num(10) < 0.0 {
            continue;
        }
        let key = (cols[2], cols[3], cols[4]);
        if current != Some(key) {
            current = Some(key);
            lines.push(Vec::new());
        }
        let (left, top) = (num(6), num(7));
        if let Some(line) = lines.last_mut() {
            line.push(Word { text: text.to_string(), left, top, right: left + num(8), bottom: top + num(9) });
        }
    }
    lines.retain(|l| !l.is_empty());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orientation_probe_counts_only_confident_words() {
        let tsv = "4\t1\t1\t1\t1\t0\t10\t20\t300\t30\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t10\t20\t120\t30\t96.1\tSehr\n\
                   5\t1\t1\t1\t1\t2\t140\t22\t170\t28\t31.0\tqxeu\n\
                   5\t1\t1\t1\t2\t1\t10\t60\t90\t30\t88.0\tHerr\n";
        assert_eq!(confident_words(tsv), "Sehr Herr");
    }

    #[test]
    fn tesseract_tsv_becomes_lines_of_words() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   1\t1\t0\t0\t0\t0\t0\t0\t800\t600\t-1\t\n\
                   4\t1\t1\t1\t1\t0\t10\t20\t300\t30\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t10\t20\t120\t30\t96.1\tSehr\n\
                   5\t1\t1\t1\t1\t2\t140\t22\t170\t28\t95.0\tgeehrter\n\
                   5\t1\t1\t1\t2\t1\t10\t60\t90\t30\t91.0\tHerr\n\
                   5\t1\t1\t1\t2\t2\t110\t60\t40\t30\t-1\t \n";
        let lines = parse_tsv(tsv);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), ["Sehr", "geehrter"]);
        assert_eq!(lines[0][1], Word { text: "geehrter".into(), left: 140.0, top: 22.0, right: 310.0, bottom: 50.0 });
        assert_eq!(lines[1].len(), 1);
        // The C API returns the same columns without a header row.
        let without_header = tsv.split_once('\n').unwrap().1;
        assert_eq!(parse_tsv(without_header), lines);
    }

    #[test]
    fn checksums_are_hex_sha256() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        for (code, fast, best) in CHECKSUMS {
            assert_eq!(code.len(), 3);
            assert!(fast.len() == 64 && best.len() == 64 && fast != best, "{code}");
        }
    }

    #[test]
    fn dpi_assumes_a4() {
        assert_eq!(assumed_dpi(&GrayImage::new(2480, 3508)), 300);
        assert_eq!(assumed_dpi(&GrayImage::new(3508, 2480)), 300);
    }
}
