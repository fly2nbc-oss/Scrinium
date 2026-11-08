use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use std::sync::LazyLock;

use anyhow::{Context, Result, anyhow, bail};
use image::{DynamicImage, ImageDecoder, ImageReader};
use pdfium_render::prelude::*;
use regex::Regex;

use crate::i18n::files;
use crate::{config, download};

/// Scans and images: rendered, straightened and read by OCR if needed.
pub const SCAN_EXTENSIONS: &[&str] = &["pdf", "png", "jpg", "jpeg", "tif", "tiff", "bmp", "webp"];
/// Text files: the content is read directly.
pub const TEXT_EXTENSIONS: &[&str] = &["md", "markdown", "txt", "csv", "tsv", "eml", "html", "htm", "rtf"];
/// Office files (ZIP with XML): Word, Excel, PowerPoint, LibreOffice.
pub const OFFICE_EXTENSIONS: &[&str] = &["docx", "docm", "dotx", "xlsx", "xlsm", "pptx", "odt", "ott", "ods", "odp"];
/// Old Office formats (binary): the text is extracted approximately.
pub const LEGACY_OFFICE_EXTENSIONS: &[&str] = &["doc", "dot", "xls", "ppt"];

/// For the file dialog: everything whose content is evaluated.
pub fn readable_extensions() -> Vec<&'static str> {
    [SCAN_EXTENSIONS, TEXT_EXTENSIONS, OFFICE_EXTENSIONS, LEGACY_OFFICE_EXTENSIONS].concat()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    Pdf,
    Image,
    Text,
    Office,
    LegacyOffice,
    /// Archives and unknown formats: only the file name is evaluated.
    Other,
}

impl DocKind {
    pub fn of(path: &Path) -> Self {
        let ext = extension(path).unwrap_or_default();
        let e = ext.as_str();
        if e == "pdf" {
            Self::Pdf
        } else if SCAN_EXTENSIONS.contains(&e) {
            Self::Image
        } else if TEXT_EXTENSIONS.contains(&e) {
            Self::Text
        } else if OFFICE_EXTENSIONS.contains(&e) {
            Self::Office
        } else if LEGACY_OFFICE_EXTENSIONS.contains(&e) {
            Self::LegacyOffice
        } else {
            Self::Other
        }
    }

    /// Has page images (preview, OCR, vision).
    pub fn has_pages(self) -> bool {
        matches!(self, Self::Pdf | Self::Image)
    }

    /// Label in the UI language.
    pub fn label(self) -> &'static str {
        self.label_in(files::t())
    }

    /// Label in a specific language, e.g. `&files::DE` for the instructions to the AI.
    pub fn label_in(self, t: &'static files::Texts) -> &'static str {
        match self {
            Self::Pdf => t.kind_pdf,
            Self::Image => t.kind_image,
            Self::Text => t.kind_text,
            Self::Office => t.kind_office,
            Self::LegacyOffice => t.kind_legacy_office,
            Self::Other => t.kind_other,
        }
    }
}

/// System files and lock files, which are never filed.
pub fn is_ignored(path: &Path) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    name.is_empty()
        || name.starts_with('.')
        || name.starts_with("~$")
        || matches!(name.as_str(), "desktop.ini" | "thumbs.db" | "ehthumbs.db" | "icon\r")
}

/// Width at which PDF pages are rendered (about 270 dpi for A4).
const PDF_RENDER_WIDTH: i32 = 2240;

/// From this many characters on, a page's text layer counts as usable.
const MIN_TEXT_LAYER_CHARS: usize = 60;
/// Minimum share of readable characters in a page's text layer. Below it, the font's character mapping is
/// broken: the page shows text, but the text layer yields symbols, replacement or special characters.
const MIN_READABLE_SHARE: f32 = 0.8;

/// Non-whitespace characters and how many of them are readable (letters, digits, common punctuation).
pub fn count_readable(text: &str) -> (usize, usize) {
    let (mut all, mut readable) = (0usize, 0usize);
    for c in text.chars().filter(|c| !c.is_whitespace()) {
        all += 1;
        if c.is_alphanumeric() || ".,;:!?-–—/()[]%€$&'\"+*=@#§_<>«»„“”‚‘’|°".contains(c) {
            readable += 1;
        }
    }
    (all, readable)
}

/// Is the text layer broken, i.e. are most of its characters unreadable?
pub fn garbled_text(text: &str) -> bool {
    let (all, readable) = count_readable(text);
    all > 0 && (readable as f32) < MIN_READABLE_SHARE * all as f32
}

/// Is a page's text layer usable? Enough characters, and most of them readable.
pub fn usable_page_text(text: &str) -> bool {
    count_readable(text).0 >= MIN_TEXT_LAYER_CHARS && !garbled_text(text)
}

pub struct LoadedDocument {
    pub kind: DocKind,
    pub pages: Vec<DynamicImage>,
    /// Text from the PDF text layer, all pages read; for text and Office files the content. Empty for images and
    /// pure scans.
    pub text_layer: String,
    /// Per page read, the text of its text layer; empty for images and pages without a text layer.
    pub page_texts: Vec<String>,
    pub page_count: usize,
    /// Per page: the text layer runs sideways (see `tall_text`). Empty if there is no text layer.
    pub tall_text: Vec<bool>,
}

impl LoadedDocument {
    /// Is this page's text layer usable (see `usable_page_text`)? If text and image pages are mixed, only some of
    /// the pages need OCR.
    pub fn page_has_text(&self, page: usize) -> bool {
        self.page_texts.get(page).is_some_and(|t| usable_page_text(t))
    }
}

/// Page images for the preview, independent of the analysis: the preview shows all pages, the analysis reads only
/// the first ones.
pub struct PagePreviews {
    pub page_count: usize,
    /// Per requested page (counted from 0), the image or the error. Pages beyond the end of the document are left out.
    pub pages: Vec<(usize, Result<DynamicImage, String>)>,
}

/// Every file can be filed; whatever is readable gets evaluated.
pub fn is_supported(path: &Path) -> bool {
    path.is_file() && !is_ignored(path)
}

pub fn extension(path: &Path) -> Option<String> {
    path.extension().map(|e| e.to_string_lossy().to_lowercase())
}

/// Holds the dynamically loaded PDFium library for the lifetime of the program.
#[derive(Default)]
pub struct PdfiumHolder {
    pdfium: Option<Pdfium>,
}

enum LoadReply<T> {
    Progress(String),
    Done(Result<T>),
}

/// A job for the loader thread; it sends back its own reply.
type LoadTask = Box<dyn FnOnce(&mut PdfiumHolder) + Send>;

/// Loader thread with the only PDFium instance (it must not change threads). Analysis and preview send it their
/// jobs; they run one after another, only the loading and rendering itself, no OCR.
static LOADER: LazyLock<std::sync::mpsc::Sender<LoadTask>> = LazyLock::new(|| {
    let (tx, rx) = std::sync::mpsc::channel::<LoadTask>();
    let started = std::thread::Builder::new().name("loader".into()).spawn(move || {
        let mut pdfium = PdfiumHolder::default();
        for task in rx {
            task(&mut pdfium);
        }
    });
    // Without a thread the sender has no receiver; every job then reports `loader_stopped`.
    if let Err(e) = started {
        eprintln!("Could not start the loader thread: {e}");
    }
    tx
});

/// Runs `job` on the loader thread and waits for it; progress messages arrive here.
fn on_loader<T: Send + 'static>(
    progress: &dyn Fn(&str),
    job: impl FnOnce(&mut PdfiumHolder, &dyn Fn(&str)) -> Result<T> + Send + 'static,
) -> Result<T> {
    let (reply, answers) = std::sync::mpsc::channel::<LoadReply<T>>();
    let task: LoadTask = Box::new(move |pdfium| {
        let report = |message: &str| {
            let _ = reply.send(LoadReply::Progress(message.to_string()));
        };
        let result = job(pdfium, &report);
        let _ = reply.send(LoadReply::Done(result));
    });
    LOADER.send(task).map_err(|_| anyhow!(files::t().loader_stopped))?;
    for answer in answers {
        match answer {
            LoadReply::Progress(message) => progress(&message),
            LoadReply::Done(result) => return result,
        }
    }
    bail!(files::t().loader_stopped)
}

/// Loads a document for the analysis on the loader thread (at most `max_pages` pages).
pub fn load_shared(path: &Path, max_pages: usize, progress: &dyn Fn(&str)) -> Result<LoadedDocument> {
    let path = path.to_path_buf();
    on_loader(progress, move |pdfium, progress| load(&path, max_pages, pdfium, progress))
}

/// Renders the pages `pages` (counted from 0) for the preview on the loader thread, the longer side at most
/// `max_side` pixels. Only a few at a time, so that a waiting analysis is not held up for long.
pub fn preview_pages(path: &Path, pages: Vec<usize>, max_side: u32) -> Result<PagePreviews> {
    let path = path.to_path_buf();
    on_loader(&|_| {}, move |pdfium, progress| render_pages(&path, &pages, max_side, pdfium, progress))
}

fn render_pages(
    path: &Path,
    pages: &[usize],
    max_side: u32,
    pdfium: &mut PdfiumHolder,
    progress: &dyn Fn(&str),
) -> Result<PagePreviews> {
    match DocKind::of(path) {
        DocKind::Pdf => {
            let pdfium = pdfium.get(progress)?;
            let doc = pdfium
                .load_pdf_from_file(path, None)
                .map_err(|e| anyhow!((files::t().pdf_not_opened)(&e.to_string())))?;
            let page_count = doc.pages().len() as usize;
            let side = max_side as i32;
            // As `to_preview` does for the analysis pages: the longer side measures `max_side`, no rotation.
            let render_cfg =
                PdfRenderConfig::new().set_target_width(side).set_maximum_width(side).set_maximum_height(side);
            let rendered = pages
                .iter()
                .filter(|&&i| i < page_count)
                .map(|&i| {
                    let render = || -> Result<DynamicImage, PdfiumError> {
                        let page = doc.pages().get(u16::try_from(i).map_err(|_| PdfiumError::PageIndexOutOfBounds)?)?;
                        let bitmap = page.render_with_config(&render_cfg)?;
                        Ok(DynamicImage::ImageRgb8(bitmap.as_image().to_rgb8()))
                    };
                    (i, render().map_err(|e| (files::t().page_not_rendered)(i + 1, &e.to_string())))
                })
                .collect();
            Ok(PagePreviews { page_count, pages: rendered })
        }
        DocKind::Image => {
            let pages = match pages.contains(&0) {
                true => vec![(0, load_image(path).map_err(|e| format!("{e:#}")))],
                false => Vec::new(),
            };
            Ok(PagePreviews { page_count: 1, pages })
        }
        _ => Ok(PagePreviews { page_count: 0, pages: Vec::new() }),
    }
}

impl PdfiumHolder {
    fn get(&mut self, progress: &dyn Fn(&str)) -> Result<&Pdfium> {
        if let Some(pdfium) = self.pdfium.take() {
            return Ok(self.pdfium.insert(pdfium));
        }
        let bindings = match bind_pdfium() {
            Ok(b) => b,
            Err(not_found) => {
                progress(files::t().loading_pdfium);
                download_pdfium(progress).map_err(|e| e.context(not_found.to_string()))?;
                bind_pdfium()?
            }
        };
        Ok(self.pdfium.insert(Pdfium::new(bindings)))
    }
}

fn pdfium_cache_dir() -> PathBuf {
    config::cache_dir().join("pdfium")
}

fn pdfium_asset() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("linux-x64"),
        ("linux", "aarch64") => Some("linux-arm64"),
        ("windows", "x86_64") => Some("win-x64"),
        ("windows", "aarch64") => Some("win-arm64"),
        ("macos", "x86_64") => Some("mac-x64"),
        ("macos", "aarch64") => Some("mac-arm64"),
        _ => None,
    }
}

/// Fetches the matching PDFium library from github.com/bblanchon/pdfium-binaries.
fn download_pdfium(progress: &dyn Fn(&str)) -> Result<()> {
    let asset = pdfium_asset().ok_or_else(|| anyhow!(files::t().no_pdfium_download))?;
    let url = format!("https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-{asset}.tgz");
    let dir = pdfium_cache_dir();
    let archive = dir.join(format!("pdfium-{asset}.tgz"));
    let cancel = AtomicBool::new(false);
    download::download_file(&url, &archive, "PDFium", &mut |p| progress(&p.describe()), &cancel)?;
    let lib_name = Pdfium::pdfium_platform_library_name();
    let result = extract_library(&archive, &lib_name.to_string_lossy(), &dir);
    std::fs::remove_file(&archive).ok();
    result
}

fn extract_library(archive: &Path, lib_name: &str, dir: &Path) -> Result<()> {
    let file = std::fs::File::open(archive)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if path.file_name().is_some_and(|n| n.to_string_lossy() == lib_name) {
            let tmp = dir.join(format!("{lib_name}.tmp"));
            entry.unpack(&tmp)?;
            std::fs::rename(&tmp, dir.join(lib_name))?;
            return Ok(());
        }
    }
    bail!((files::t().library_not_in_package)(lib_name))
}

fn pdfium_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(p) = std::env::var("SCRINIUM_PDFIUM") {
        let p = p.trim();
        if !p.is_empty() {
            let p = PathBuf::from(p);
            dirs.push(if p.is_file() { p.parent().map(Path::to_path_buf).unwrap_or_default() } else { p });
        }
    }
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        dirs.push(exe_dir.join("lib"));
        dirs.push(exe_dir);
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("lib"));
        dirs.push(cwd);
    }
    dirs.push(pdfium_cache_dir());
    dirs
}

fn bind_pdfium() -> Result<Box<dyn PdfiumLibraryBindings>> {
    for dir in pdfium_search_dirs() {
        let candidate = Pdfium::pdfium_platform_library_name_at_path(&dir);
        if candidate.is_file()
            && let Ok(b) = Pdfium::bind_to_library(&candidate)
        {
            return Ok(b);
        }
    }
    Pdfium::bind_to_system_library()
        .map_err(|_| anyhow!((files::t().pdfium_not_found)(&Pdfium::pdfium_platform_library_name().to_string_lossy())))
}

pub fn load(
    path: &Path,
    max_pages: usize,
    pdfium: &mut PdfiumHolder,
    progress: &dyn Fn(&str),
) -> Result<LoadedDocument> {
    let kind = DocKind::of(path);
    let text_only = |text: String| LoadedDocument {
        kind,
        pages: Vec::new(),
        text_layer: text,
        page_texts: Vec::new(),
        page_count: 0,
        tall_text: Vec::new(),
    };
    match kind {
        DocKind::Pdf => load_pdf(path, max_pages.max(1), pdfium.get(progress)?),
        DocKind::Image => {
            let img = load_image(path)?;
            Ok(LoadedDocument {
                kind,
                pages: vec![img],
                text_layer: String::new(),
                page_texts: Vec::new(),
                page_count: 1,
                tall_text: Vec::new(),
            })
        }
        DocKind::Text => Ok(text_only(text_file(path)?)),
        DocKind::Office => Ok(text_only(office_text(path)?)),
        DocKind::LegacyOffice => Ok(text_only(legacy_office_text(&std::fs::read(path)?))),
        DocKind::Other => Ok(text_only(archive_listing(path).unwrap_or_default())),
    }
}

/// Largest amount of text read from a file (enough for header, subject and content).
const MAX_TEXT_BYTES: u64 = 4 * 1024 * 1024;

fn read_limited(path: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut buf = Vec::new();
    std::fs::File::open(path)
        .with_context(|| (files::t().not_readable)(&path.display().to_string()))?
        .take(MAX_TEXT_BYTES)
        .read_to_end(&mut buf)?;
    Ok(buf)
}

fn text_file(path: &Path) -> Result<String> {
    let text = decode_text(&read_limited(path)?);
    Ok(match extension(path).as_deref() {
        Some("html" | "htm") => html_to_text(&text),
        Some("rtf") => rtf_to_text(&text),
        _ => text,
    })
}

/// UTF-8 (also with BOM), otherwise Windows-1252 – so older German text files read correctly.
pub fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| cp1252(b)).collect(),
    }
}

fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘',
        '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9f => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

static TAG: LazyLock<Regex> = LazyLock::new(|| crate::heuristics::code_regex(r"<[^>]*>"));

fn unescape_xml(s: &str) -> String {
    static ENTITY: LazyLock<Regex> =
        LazyLock::new(|| crate::heuristics::code_regex(r"&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);"));
    ENTITY
        .replace_all(s, |c: &regex::Captures| {
            let e = &c[1];
            let code = if let Some(hex) = e.strip_prefix("#x") {
                u32::from_str_radix(hex, 16).ok()
            } else if let Some(dec) = e.strip_prefix('#') {
                dec.parse().ok()
            } else {
                None
            };
            if let Some(ch) = code.and_then(char::from_u32) {
                return ch.to_string();
            }
            match e {
                "amp" => "&",
                "lt" => "<",
                "gt" => ">",
                "quot" => "\"",
                "apos" => "'",
                "nbsp" => " ",
                "auml" => "ä",
                "ouml" => "ö",
                "uuml" => "ü",
                "Auml" => "Ä",
                "Ouml" => "Ö",
                "Uuml" => "Ü",
                "szlig" => "ß",
                "euro" => "€",
                _ => "",
            }
            .to_string()
        })
        .into_owned()
}

/// Makes a PDF's text layer readable: PDFium returns hyphenation as the control character U+0002 in the middle of
/// a word ("Kon\u{2}tostand"), plus Windows line endings. Hyphenated words are joined again, control characters
/// removed, lines trimmed and blank lines collapsed.
pub fn clean_pdf_text(raw: &str) -> String {
    let joined = raw.replace("\u{2}\r\n", "").replace("\u{2}\n", "").replace(['\u{2}', '\u{ad}'], "");
    let unified = joined.replace("\r\n", "\n").replace('\r', "\n");
    let visible: String = unified.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t').collect();
    tidy_lines(&visible)
}

/// Collapses blank lines, trims lines.
fn tidy_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank = 0;
    for line in s.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")) {
        if line.is_empty() {
            blank += 1;
            if blank == 1 && !out.is_empty() {
                out.push('\n');
            }
            continue;
        }
        blank = 0;
        out.push_str(&line);
        out.push('\n');
    }
    out.trim().to_string()
}

/// Text from Office XML: paragraphs, line breaks, table cells and slides stay recognisable.
pub fn xml_to_text(xml: &str) -> String {
    static BREAKS: LazyLock<Regex> = LazyLock::new(|| {
        crate::heuristics::code_regex(
            r"(?x)
            </w:p> | <w:br\b[^>]*/> | <w:cr\b[^>]*/> | </w:tr> |
            </text:p> | </text:h> | <text:line-break\b[^>]*/> | </table:table-row> |
            </a:p> | </si>",
        )
    });
    static TABS: LazyLock<Regex> = LazyLock::new(|| {
        crate::heuristics::code_regex(r"<w:tab\b[^>]*/>|</w:tc>|<text:tab\b[^>]*/>|</table:table-cell>")
    });
    static SPACES: LazyLock<Regex> = LazyLock::new(|| crate::heuristics::code_regex(r"<text:s\b[^>]*/>"));
    // Field codes (PAGE \* MERGEFORMAT) and deleted text from change tracking are not content.
    static HIDDEN: LazyLock<Regex> = LazyLock::new(|| {
        crate::heuristics::code_regex(r"(?s)<w:instrText\b[^>]*>.*?</w:instrText>|<w:delText\b[^>]*>.*?</w:delText>")
    });
    let s = HIDDEN.replace_all(xml, "");
    let s = BREAKS.replace_all(&s, "\n");
    let s = TABS.replace_all(&s, "\t");
    let s = SPACES.replace_all(&s, " ");
    tidy_lines(&unescape_xml(&TAG.replace_all(&s, "")))
}

fn html_to_text(html: &str) -> String {
    static DROP: LazyLock<Regex> =
        LazyLock::new(|| crate::heuristics::code_regex(r"(?is)<(script|style|head)\b.*?</(script|style|head)>"));
    static BLOCK: LazyLock<Regex> =
        LazyLock::new(|| crate::heuristics::code_regex(r"(?i)<br\s*/?>|</(p|div|h[1-6]|li|tr|table|section|article)>"));
    let s = DROP.replace_all(html, "");
    let s = BLOCK.replace_all(&s, "\n");
    tidy_lines(&unescape_xml(&TAG.replace_all(&s, "")))
}

/// RTF without formatting: control words and groups removed, `\'e4` as a Windows-1252 character.
pub fn rtf_to_text(rtf: &str) -> String {
    static HIDDEN: LazyLock<Regex> = LazyLock::new(|| {
        crate::heuristics::code_regex(
            r"\{\\\*[^{}]*\}|\{\\(fonttbl|colortbl|stylesheet|info)[^{}]*(\{[^{}]*\}[^{}]*)*\}",
        )
    });
    static HEX: LazyLock<Regex> = LazyLock::new(|| crate::heuristics::code_regex(r"\\'([0-9a-fA-F]{2})"));
    static UNI: LazyLock<Regex> = LazyLock::new(|| crate::heuristics::code_regex(r"\\u(-?\d+)\??"));
    static PAR: LazyLock<Regex> = LazyLock::new(|| crate::heuristics::code_regex(r"\\(par|line)\b ?"));
    static CONTROL: LazyLock<Regex> = LazyLock::new(|| crate::heuristics::code_regex(r"\\[a-zA-Z]+-?\d* ?"));
    let s = HIDDEN.replace_all(rtf, "");
    let s = PAR.replace_all(&s, "\n");
    let s = UNI.replace_all(&s, |c: &regex::Captures| {
        c[1].parse::<i32>()
            .ok()
            .and_then(|n| char::from_u32(n.rem_euclid(65536) as u32))
            .map(String::from)
            .unwrap_or_default()
    });
    let s = HEX.replace_all(&s, |c: &regex::Captures| {
        u8::from_str_radix(&c[1], 16).map(|b| cp1252(b).to_string()).unwrap_or_default()
    });
    let s = CONTROL.replace_all(&s, "");
    tidy_lines(&s.replace(['{', '}'], ""))
}

/// Reads the text parts of an Office document in reading order (for Word, headers first,
/// because that is often where the sender is).
pub fn office_text(path: &Path) -> Result<String> {
    let t = files::t();
    let file = std::fs::File::open(path).with_context(|| (t.not_readable)(&path.display().to_string()))?;
    let mut zip = zip::ZipArchive::new(file).context(t.office_damaged)?;
    // Names that cannot be decoded do not belong to any text part.
    let names: Vec<String> = zip.file_names().filter_map(|n| n.ok().map(|n| n.into_owned())).collect();
    let numbered = |prefix: &str| {
        let mut v: Vec<(u32, String)> = names
            .iter()
            .filter_map(|n| {
                let rest = n.strip_prefix(prefix)?.strip_suffix(".xml")?;
                Some((rest.parse().ok()?, n.clone()))
            })
            .collect();
        v.sort();
        v.into_iter().map(|(_, n)| n).collect::<Vec<_>>()
    };
    let ext = extension(path).unwrap_or_default();
    let parts: Vec<String> = match ext.as_str() {
        "docx" | "docm" | "dotx" => {
            let mut p = numbered("word/header");
            p.push("word/document.xml".into());
            p.extend(numbered("word/footer"));
            p
        }
        "xlsx" | "xlsm" => vec!["xl/workbook.xml".into(), "xl/sharedStrings.xml".into()],
        "pptx" => numbered("ppt/slides/slide"),
        _ => vec!["content.xml".into()],
    };
    let mut out = String::new();
    for name in parts {
        let Ok(entry) = zip.by_name(&name) else { continue };
        let mut xml = String::new();
        use std::io::Read;
        entry.take(MAX_TEXT_BYTES * 4).read_to_string(&mut xml).ok();
        let text = if name == "xl/workbook.xml" { sheet_names(&xml) } else { xml_to_text(&xml) };
        if !text.trim().is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(text.trim());
        }
    }
    Ok(out)
}

fn sheet_names(workbook: &str) -> String {
    static SHEET: LazyLock<Regex> = LazyLock::new(|| crate::heuristics::code_regex(r#"<sheet\b[^>]*\bname="([^"]*)""#));
    let names: Vec<String> = SHEET.captures_iter(workbook).map(|c| unescape_xml(&c[1])).collect();
    if names.is_empty() { String::new() } else { format!("Tabellenblätter: {}", names.join(", ")) }
}

/// Text from old Word, Excel and PowerPoint files: contiguous readable character runs
/// (UTF-16 and Windows-1252). Imprecise, but enough for sender, date and subject.
pub fn legacy_office_text(bytes: &[u8]) -> String {
    let readable = |c: char| c.is_alphanumeric() || " .,;:-–/()€%&'\"!?§+@\t".contains(c);
    let mut runs: Vec<String> = Vec::new();
    let mut push = |run: &mut String, min: usize| {
        let t = run.trim();
        if t.chars().filter(|c| c.is_alphabetic()).count() >= min {
            runs.push(t.to_string());
        }
        run.clear();
    };
    // UTF-16LE
    let mut run = String::new();
    for pair in bytes.as_chunks::<2>().0 {
        let c = char::from_u32(u16::from_le_bytes(*pair) as u32).unwrap_or('\0');
        if readable(c) {
            run.push(c);
        } else {
            push(&mut run, 4);
        }
    }
    push(&mut run, 4);
    // 8-bit text (Word often stores text compressed as Windows-1252)
    for &b in bytes {
        let c = cp1252(b);
        if b >= 0x20 && readable(c) {
            run.push(c);
        } else {
            push(&mut run, 6);
        }
    }
    push(&mut run, 6);
    let mut seen = std::collections::HashSet::new();
    let text: Vec<String> = runs.into_iter().filter(|r| seen.insert(r.clone())).collect();
    let joined = text.join("\n");
    joined.chars().take(30_000).collect()
}

/// Table of contents of a ZIP archive (a good hint for archives without readable text).
fn archive_listing(path: &Path) -> Option<String> {
    if extension(path).as_deref() != Some("zip") {
        return None;
    }
    let zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let names: Vec<String> = zip.file_names().take(200).filter_map(|n| n.ok().map(|n| n.into_owned())).collect();
    Some(format!("ZIP-Archiv mit {} Dateien:\n{}", zip.len(), names.join("\n")))
}

/// Loads an image and applies the EXIF orientation (phone photos, some scanners).
pub fn load_image(path: &Path) -> Result<DynamicImage> {
    let t = files::t();
    let reader = ImageReader::open(path)
        .with_context(|| (t.not_readable)(&path.display().to_string()))?
        .with_guessed_format()?;
    let mut decoder = reader.into_decoder().context(t.image_format_unknown)?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).context(t.image_not_decoded)?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Ok(img)
}

fn load_pdf(path: &Path, max_pages: usize, pdfium: &Pdfium) -> Result<LoadedDocument> {
    let doc =
        pdfium.load_pdf_from_file(path, None).map_err(|e| anyhow!((files::t().pdf_not_opened)(&e.to_string())))?;
    let page_count = doc.pages().len() as usize;
    let render_cfg = PdfRenderConfig::new().set_target_width(PDF_RENDER_WIDTH).set_maximum_height(PDF_RENDER_WIDTH * 2);

    let mut pages = Vec::new();
    let mut page_texts = Vec::new();
    let mut tall = Vec::new();
    for (i, page) in doc.pages().iter().enumerate().take(max_pages) {
        tall.push(tall_text(&page));
        page_texts.push(page.text().map(|text| clean_pdf_text(&text.all())).unwrap_or_default());
        let bitmap = page
            .render_with_config(&render_cfg)
            .map_err(|e| anyhow!((files::t().page_not_rendered)(i + 1, &e.to_string())))?;
        pages.push(DynamicImage::ImageRgb8(bitmap.as_image().to_rgb8()));
    }
    if pages.is_empty() {
        bail!(files::t().pdf_no_pages);
    }
    let text_layer = page_texts.iter().filter(|t| !t.is_empty()).cloned().collect::<Vec<_>>().join("\n\n");
    Ok(LoadedDocument { kind: DocKind::Pdf, pages, text_layer, page_texts, page_count, tall_text: tall })
}

/// From this share of tall text segments on, a page is suspected to be sideways.
const TALL_TEXT_SHARE: f32 = 0.8;
/// At least this many tall segments; a few rotated labels (title block of a drawing) do not count.
const TALL_TEXT_MIN: usize = 10;

/// Does the page's text run sideways as the page is displayed? Then the text segments (lines, words) are tall and
/// narrow. Scan apps often put the characters of sideways pages unrotated into such boxes, so their angle does not
/// reveal the orientation, nor do the rotated title blocks of drawings. OCR therefore has to decide; this is only
/// the trigger.
fn tall_text(page: &PdfPage) -> bool {
    let Ok(text) = page.text() else { return false };
    // The boxes are in page coordinates; a page rotation by 90° or 270° swaps tall and wide.
    let turned = matches!(page.rotation(), Ok(PdfPageRenderRotation::Degrees90 | PdfPageRenderRotation::Degrees270));
    let (mut tall, mut wide) = (0usize, 0usize);
    for segment in text.segments().iter() {
        let bounds = segment.bounds();
        let (w, h) = (bounds.width().value, bounds.height().value);
        // Single characters say nothing about the direction.
        if w.max(h) < 15.0 {
            continue;
        }
        if (h > w) != turned {
            tall += 1;
        } else {
            wide += 1;
        }
    }
    tall >= TALL_TEXT_MIN && tall as f32 >= TALL_TEXT_SHARE * (tall + wide) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_and_ignored_files() {
        assert_eq!(DocKind::of(Path::new("scan.PDF")), DocKind::Pdf);
        assert_eq!(DocKind::of(Path::new("a/b/foto.jpeg")), DocKind::Image);
        assert_eq!(DocKind::of(Path::new("notiz.md")), DocKind::Text);
        assert_eq!(DocKind::of(Path::new("Brief.docx")), DocKind::Office);
        assert_eq!(DocKind::of(Path::new("Brief.ODT")), DocKind::Office);
        assert_eq!(DocKind::of(Path::new("alt.doc")), DocKind::LegacyOffice);
        assert_eq!(DocKind::of(Path::new("Sonstiges.7z")), DocKind::Other);
        assert!(
            is_ignored(Path::new("x/desktop.ini"))
                && is_ignored(Path::new("~$Brief.docx"))
                && is_ignored(Path::new(".DS_Store"))
        );
        assert!(!is_ignored(Path::new("Brief.docx")));
    }

    fn write_zip(path: &Path, files: &[(&str, &str)]) {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, data) in files {
            zip.start_file(
                *name,
                zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            zip.write_all(data.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn reads_office_and_text_files() {
        let dir = std::env::temp_dir().join(format!("scrinium-office-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let docx = dir.join("Brief.docx");
        write_zip(
            &docx,
            &[
                ("word/header1.xml", r#"<w:hdr><w:p><w:r><w:t>Kanzlei Bauer &amp; Partner</w:t></w:r></w:p></w:hdr>"#),
                (
                    "word/document.xml",
                    r#"<w:document><w:body><w:p><w:r><w:t>Ulm, den 04.10.2010</w:t></w:r></w:p><w:p><w:r><w:t>Betreff:</w:t></w:r><w:r><w:tab/><w:t>Mietsache</w:t></w:r><w:r><w:delText>Pacht</w:delText></w:r></w:p></w:body></w:document>"#,
                ),
                (
                    "word/footer1.xml",
                    r#"<w:ftr><w:p><w:r><w:t xml:space="preserve">Seite </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGE \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t>2</w:t></w:r></w:p></w:ftr>"#,
                ),
            ],
        );
        let text = office_text(&docx).unwrap();
        assert_eq!(text, "Kanzlei Bauer & Partner\n\nUlm, den 04.10.2010\nBetreff: Mietsache\n\nSeite 2");
        let odt = dir.join("Brief.odt");
        write_zip(
            &odt,
            &[(
                "content.xml",
                r#"<office:text><text:h>Vollmacht</text:h><text:p>Hiermit<text:s/>bevollmächtige ich</text:p></office:text>"#,
            )],
        );
        assert_eq!(office_text(&odt).unwrap(), "Vollmacht\nHiermit bevollmächtige ich");
        let xlsx = dir.join("Liste.xlsx");
        write_zip(
            &xlsx,
            &[
                ("xl/workbook.xml", r#"<workbook><sheets><sheet name="Händler" sheetId="1"/></sheets></workbook>"#),
                ("xl/sharedStrings.xml", r#"<sst><si><t>Name</t></si><si><t>Preis &#8364;</t></si></sst>"#),
            ],
        );
        assert_eq!(office_text(&xlsx).unwrap(), "Tabellenblätter: Händler\n\nName\nPreis €");

        let md = dir.join("Notiz.md");
        std::fs::write(&md, "# Netzwerk\r\nStand: 19.09.2026").unwrap();
        let doc = load(&md, 3, &mut PdfiumHolder::default(), &|_| {}).unwrap();
        assert_eq!(doc.kind, DocKind::Text);
        assert!(doc.pages.is_empty() && doc.page_texts.is_empty());
        assert!(doc.text_layer.contains("Stand: 19.09.2026"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn pdf_text_is_readable() {
        let raw = "Kon\u{2}tostand  am  Monatsende\r\nInanspruchnah\u{2}\r\nme\r\n\r\n\r\n\u{7}Seite 2\r\n";
        assert_eq!(clean_pdf_text(raw), "Kontostand am Monatsende\nInanspruchnahme\n\nSeite 2");
    }

    #[test]
    fn decodes_legacy_text() {
        assert_eq!(decode_text(b"Gr\xfc\xdfe \x80"), "Grüße €");
        assert_eq!(decode_text("\u{feff}Grüße".as_bytes()), "Grüße");
        assert_eq!(
            rtf_to_text(r"{\rtf1\ansi{\fonttbl{\f0 Arial;}}\f0 Sehr geehrte Frau M\'fcller,\par Ihr Antrag}"),
            "Sehr geehrte Frau Müller,\nIhr Antrag"
        );
        let mut doc = vec![0u8; 64];
        for c in "Mietvertrag vom 01.02.2012".encode_utf16() {
            doc.extend(c.to_le_bytes());
        }
        doc.extend([0u8; 16]);
        assert!(legacy_office_text(&doc).contains("Mietvertrag vom 01.02.2012"));
        assert!(
            html_to_text(
                "<html><head><title>x</title></head><body><p>Hallo&nbsp;Welt</p><script>y()</script></body></html>"
            )
            .starts_with("Hallo Welt")
        );
    }

    #[test]
    fn loads_png_image() {
        let dir = std::env::temp_dir().join(format!("scrinium-ingest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("seite.png");
        image::RgbImage::from_pixel(40, 30, image::Rgb([255, 255, 255])).save(&p).unwrap();
        let doc = load(&p, 3, &mut PdfiumHolder::default(), &|_| {}).unwrap();
        assert_eq!(doc.kind, DocKind::Image);
        assert_eq!(doc.pages.len(), 1);
        assert_eq!(doc.pages[0].width(), 40);
        assert!(!doc.page_has_text(0));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn text_layer_counts_per_page() {
        let letter = "Sehr geehrte Frau Müller, anbei erhalten Sie die Rechnung Nr. 4711 vom 03.10.2026 über 120,00 €.";
        assert!(usable_page_text(letter));
        // An image page with a stamp or page number in the text layer is not a text page.
        assert!(!usable_page_text("Seite 2 von 3"));
        assert!(!usable_page_text(""));
        // Broken character mapping: the page shows text, the text layer yields replacement and special characters.
        let broken: String = letter.chars().map(|c| if c.is_alphabetic() { '\u{e01a}' } else { c }).collect();
        assert!(!usable_page_text(&broken) && garbled_text(&broken));
        assert!(!usable_page_text(&"\u{fffd}".repeat(80)));
        // Short but readable is not broken: the page keeps the text if OCR finds no more.
        assert!(!garbled_text("Seite 2 von 3") && !garbled_text(""));

        // Mixed PDF: page 1 with a text layer, page 2 a scan without.
        let doc = LoadedDocument {
            kind: DocKind::Pdf,
            pages: vec![DynamicImage::new_rgb8(1, 1), DynamicImage::new_rgb8(1, 1)],
            text_layer: letter.to_string(),
            page_texts: vec![letter.to_string(), String::new()],
            page_count: 2,
            tall_text: vec![false, false],
        };
        assert!(doc.page_has_text(0) && !doc.page_has_text(1) && !doc.page_has_text(2));
    }

    #[test]
    fn extracts_library_from_release_archive() {
        let dir = std::env::temp_dir().join(format!("scrinium-pdfium-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let archive = dir.join("pdfium.tgz");
        let gz = flate2::write::GzEncoder::new(std::fs::File::create(&archive).unwrap(), flate2::Compression::fast());
        let mut builder = tar::Builder::new(gz);
        for (name, data) in [("include/fpdfview.h", &b"header"[..]), ("lib/libpdfium.so", &b"ELF-Bibliothek"[..])] {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, name, data).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap();
        extract_library(&archive, "libpdfium.so", &dir).unwrap();
        assert_eq!(std::fs::read(dir.join("libpdfium.so")).unwrap(), b"ELF-Bibliothek");
        assert!(extract_library(&archive, "pdfium.dll", &dir).is_err());
        std::fs::remove_dir_all(dir).ok();
    }
}
