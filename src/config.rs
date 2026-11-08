use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::{ProjectDirs, UserDirs};
use serde::{Deserialize, Serialize};

use crate::i18n::settings::t;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Root folder of the archive. All target folders are relative to it.
    pub archive_root: String,
    /// Placeholders: {date} {doctype} {sender} {subject} {person} {extra}, plus {year} {month} {day}. {title} is an
    /// older name for {subject}. Empty parts are dropped together with their separator.
    pub filename_template: String,
    /// Date format for {date}; an unknown day or month becomes `00` (`160100`), without a date {date} is dropped.
    pub date_format: String,
    /// Character rules: spaces become `-`, letters, digits, `-` and `_` are allowed.
    pub compact_filenames: bool,
    /// Maximum length of the file name without extension. Names that are too long shorten the subject first.
    pub max_filename_length: usize,
    /// How deep the existing folder structure is read.
    pub scan_depth: usize,
    /// Folders that are neither offered nor read (including subfolders).
    pub exclude: Vec<String>,
    /// Collection folders (older setting): the folder itself is offered to the AI, its subfolders are not. The settings
    /// take this over by setting the subfolders to "not used".
    pub collapse: Vec<String>,
    /// May the AI suggest new folders? Year folders are always created.
    pub allow_new_folders: bool,
    /// Folder used if neither a rule nor the AI finds anything suitable ("" = root).
    pub fallback_folder: String,
    /// Copy the original file instead of moving it.
    pub copy_instead_of_move: bool,
    /// Write title, subject, tags and comment into the file when filing (PDF, Office, images), so that Windows Search
    /// finds them.
    pub write_tags: bool,
    /// Record the suggestion and the filed values of each filing in `decisions.jsonl` next to the configuration,
    /// to improve keywords, folder descriptions and rules (`scrinium --report`).
    pub log_decisions: bool,
    /// Maximum number of pages analysed per document.
    pub max_pages: usize,
    /// Extensions of program files that are never renamed (`est*` = everything starting with est).
    pub keep_name_extensions: Vec<String>,
    /// Fixed list of doctypes. Empty = free terms.
    pub doctypes: Vec<String>,
    /// Fixed suffixes at the end of the name, e.g. `unterschrieben`.
    pub extras: Vec<String>,
    /// Persons with their spellings (aliases become the name).
    pub persons: Vec<Alias>,
    /// Person in the file name only if the target folder belongs to them (name in the path, e.g. `Gehalt/2026_Max`,
    /// `Familie/Erika`). A rule with `person` always applies.
    pub person_from_folder: bool,
    /// Short names for senders, e.g. `MuV` for "M+V Allgemeine Versicherung AG".
    pub senders: Vec<Alias>,
    /// Different file name templates for certain folders (e.g. manuals without a date).
    pub folder_templates: Vec<FolderTemplate>,
    /// Short description per folder; helps the AI in ambiguous cases.
    pub folder_notes: BTreeMap<String, String>,
    /// Keywords per folder, only when needed: "Wort, Wort & Wort, -Gegenwort". They apply when the folder is chosen
    /// step by step, also for the parent folders; `{year}` in the key stands for a year series (`Gehalt/{year}_Max`).
    pub folder_words: BTreeMap<String, String>,
    pub ocr: OcrConfig,
    pub llm: LlmConfig,
    pub scan: ScanConfig,
    pub rules: Vec<Rule>,
}

/// Scanning. `device`: empty = automatic (Windows: the TWAIN scanner, otherwise the Windows dialog; Linux: the first
/// SANE scanner), otherwise `twain:Name`, `wia` or `sane:Device`. Resolution and color apply to SANE only; TWAIN and
/// WIA ask in their own dialog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanConfig {
    pub device: String,
    /// Where new scans go until they are filed ("" = the Downloads folder).
    pub folder: String,
    pub resolution: u32,
    pub color: bool,
    /// How the pages are processed; chosen with *Scan as* below *Scan* before scanning.
    pub preset: ScanPreset,
    /// Put the recognized text invisibly into the PDF so that it is searchable.
    pub text_layer: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            device: String::new(),
            folder: String::new(),
            resolution: 300,
            color: true,
            preset: ScanPreset::default(),
            text_layer: true,
        }
    }
}

/// Processing of scanned pages. Pages are straightened for all presets except *Photo*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScanPreset {
    /// Black on white (1 bit): crisp text, smallest files.
    #[serde(rename = "bw")]
    BlackWhite,
    /// Color, the paper becomes white.
    #[default]
    #[serde(rename = "color")]
    Color,
    /// Printed in color: colors stay, a little more contrast.
    #[serde(rename = "brochure")]
    Brochure,
    /// Unchanged in high quality, not rotated.
    #[serde(rename = "photo")]
    Photo,
}

impl ScanPreset {
    pub const ALL: [ScanPreset; 4] =
        [ScanPreset::BlackWhite, ScanPreset::Color, ScanPreset::Brochure, ScanPreset::Photo];

    pub fn label(self) -> &'static str {
        let t = t();
        match self {
            ScanPreset::BlackWhite => t.preset_bw,
            ScanPreset::Color => t.preset_color,
            ScanPreset::Brochure => t.preset_brochure,
            ScanPreset::Photo => t.preset_photo,
        }
    }

    pub fn hint(self) -> &'static str {
        let t = t();
        match self {
            ScanPreset::BlackWhite => t.preset_bw_hint,
            ScanPreset::Color => t.preset_color_hint,
            ScanPreset::Brochure => t.preset_brochure_hint,
            ScanPreset::Photo => t.preset_photo_hint,
        }
    }
}

impl ScanConfig {
    /// The scan folder: the configured one, otherwise Downloads (as a fallback the Scrinium cache).
    pub fn folder_path(&self) -> PathBuf {
        match self.folder.trim() {
            "" => default_scan_folder(),
            f => PathBuf::from(f),
        }
    }
}

pub fn default_scan_folder() -> PathBuf {
    UserDirs::new().and_then(|d| d.download_dir().map(Path::to_path_buf)).unwrap_or_else(|| cache_dir().join("Scans"))
}

/// A name with further spellings, for persons and senders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Alias {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

impl Alias {
    /// The whole (already normalized) word is the name or one of its spellings.
    pub fn matches_word(&self, normalized_word: &str) -> bool {
        std::iter::once(&self.name).chain(&self.aliases).any(|a| crate::archive::normalize(a.trim()) == normalized_word)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FolderTemplate {
    /// Applies to this folder and all below it.
    pub folder: String,
    pub template: String,
}

/// Tesseract language data: `tessdata_fast` (small, fast) or `tessdata_best` (more accurate, about three times slower).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OcrQuality {
    #[default]
    Fast,
    Best,
}

/// Text recognition with Tesseract. Earlier keys (`engine`, `tesseract_path`) are skipped when reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OcrConfig {
    /// Languages of the documents as Tesseract codes, joined with "+", e.g. "deu+eng".
    #[serde(alias = "tesseract_lang")]
    pub languages: String,
    pub quality: OcrQuality,
}

impl OcrConfig {
    /// The languages one by one, without empty ones and duplicates.
    pub fn language_list(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for code in self.languages.split(['+', ',', ' ']).map(str::trim).filter(|c| !c.is_empty()) {
            if !out.iter().any(|c| c == code) {
                out.push(code.to_string());
            }
        }
        out
    }

    pub fn set_languages(&mut self, codes: &[String]) {
        self.languages = codes.join("+");
    }

    /// For a new configuration: the system language and English.
    pub fn for_system() -> Self {
        let own = match crate::i18n::system_default() {
            crate::i18n::Lang::De => "deu",
            crate::i18n::Lang::Fr => "fra",
            crate::i18n::Lang::Es => "spa",
            crate::i18n::Lang::It => "ita",
            crate::i18n::Lang::En => "eng",
        };
        let languages = if own == "eng" { "eng".to_string() } else { format!("{own}+eng") };
        Self { languages, ..Self::default() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LlmBackend {
    /// llama.cpp inside the program, models are downloaded in the app.
    #[default]
    Embedded,
    /// External OpenAI-compatible server (LM Studio, Ollama …).
    Server,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    pub enabled: bool,
    pub backend: LlmBackend,
    /// Catalog model of the embedded AI; empty = automatic, matching the GPU.
    pub model_id: String,
    /// Own GGUF file instead of a catalog model (optional).
    pub model_path: String,
    /// Matching image projector (mmproj-*.gguf) for the own file (optional).
    pub mmproj_path: String,
    /// Layers on the GPU: -1 = all, 0 = CPU only.
    pub gpu_layers: i32,
    /// Context length in tokens (image + text + answer).
    pub context_size: u32,
    /// OpenAI-compatible endpoint (LM Studio, Ollama, llama.cpp server …)
    pub base_url: String,
    /// Empty = use the first loaded model.
    pub model: String,
    pub api_key: String,
    /// Send the page image to a vision model as well.
    pub use_vision: bool,
    pub timeout_secs: u64,
    /// Maximum characters of OCR text in the prompt.
    pub max_chars: usize,
    /// Hints for the file name details (sender, subject …). Appended to the instructions.
    pub instructions: String,
    /// In steps: first the AI describes the document, then it chooses the folder level by level after the folder
    /// descriptions, last the details for the file name. Otherwise a single request with the whole folder list.
    pub folder_steps: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// A missing field in older files stays enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Applies if one of the keywords occurs in the text (ignoring case).
    pub keywords: Vec<String>,
    /// Applies only if all keywords occur.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub match_all: bool,
    /// Does not apply if one of these keywords occurs.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exclude_keywords: Vec<String>,
    /// Applies only if all of these keywords occur as well (e.g. one's own town).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub require_keywords: Vec<String>,
    /// Target folder relative to the archive, may contain {year}, {month} and {day}.
    pub folder: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doctype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub person: Option<String>,
}

fn default_enabled() -> bool {
    true
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            name: String::new(),
            enabled: true,
            keywords: Vec::new(),
            match_all: false,
            exclude_keywords: Vec::new(),
            require_keywords: Vec::new(),
            folder: String::new(),
            doctype: None,
            sender: None,
            person: None,
        }
    }
}

impl Rule {
    /// Display name: own name or the first keywords.
    pub fn display_name(&self) -> String {
        if !self.name.trim().is_empty() {
            return self.name.trim().to_string();
        }
        let kws: Vec<&str> = self.keywords.iter().map(|k| k.trim()).filter(|k| !k.is_empty()).take(3).collect();
        if kws.is_empty() { t().new_rule.to_string() } else { kws.join(", ") }
    }
}

impl Default for Config {
    fn default() -> Self {
        let archive_root = UserDirs::new()
            .and_then(|u| u.document_dir().map(|d| d.join("Ablage")))
            .unwrap_or_else(|| PathBuf::from("Ablage"));
        Self {
            archive_root: archive_root.to_string_lossy().into_owned(),
            filename_template: "{date}_{doctype}_{sender}_{subject}_{person}_{extra}".into(),
            date_format: "%y%m%d".into(),
            compact_filenames: true,
            max_filename_length: 80,
            scan_depth: 3,
            exclude: Vec::new(),
            collapse: Vec::new(),
            allow_new_folders: false,
            fallback_folder: "00_Eingang".into(),
            copy_instead_of_move: false,
            write_tags: true,
            log_decisions: true,
            max_pages: 3,
            keep_name_extensions: ["pfx", "p12", "kdbx", "knxproj", "s07", "est*", "gew*", "ust*"]
                .map(String::from)
                .to_vec(),
            doctypes: crate::doclang::current().default_doctypes.iter().map(|s| s.to_string()).collect(),
            extras: ["unterschrieben", "Entwurf", "Kopie", "sw", "Anlage-1", "v2"].map(String::from).to_vec(),
            persons: Vec::new(),
            person_from_folder: false,
            senders: Vec::new(),
            folder_templates: Vec::new(),
            folder_notes: BTreeMap::new(),
            folder_words: BTreeMap::new(),
            ocr: OcrConfig::default(),
            llm: LlmConfig::default(),
            scan: ScanConfig::default(),
            rules: Vec::new(),
        }
    }
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self { languages: "deu+eng".into(), quality: OcrQuality::Fast }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: LlmBackend::Embedded,
            model_id: String::new(),
            model_path: String::new(),
            mmproj_path: String::new(),
            gpu_layers: -1,
            context_size: 12288,
            base_url: "http://localhost:1234/v1".into(),
            model: String::new(),
            api_key: String::new(),
            use_vision: true,
            timeout_secs: 120,
            max_chars: 6000,
            instructions: String::new(),
            folder_steps: true,
        }
    }
}

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("de", "scrinium", "Scrinium")
}

/// `SCRINIUM_CONFIG` overrides the path of the configuration file.
pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("SCRINIUM_CONFIG") {
        let p = p.trim();
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    project_dirs().map(|d| d.config_dir().join("config.toml")).unwrap_or_else(|| PathBuf::from("scrinium.toml"))
}

pub fn cache_dir() -> PathBuf {
    project_dirs().map(|d| d.cache_dir().to_path_buf()).unwrap_or_else(|| std::env::temp_dir().join("scrinium"))
}

impl Config {
    /// Loads the configuration or creates a default file on first start.
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if path.exists() {
            let shown = path.display().to_string();
            let raw = std::fs::read_to_string(path).with_context(|| (t().config_unreadable)(&shown))?;
            let cfg: Config = toml::from_str(&raw).with_context(|| (t().config_invalid)(&shown))?;
            Ok(cfg)
        } else {
            // Text recognition initially reads the system language and English.
            let cfg = Config { ocr: OcrConfig::for_system(), ..Config::default() };
            cfg.save(path)?;
            Ok(cfg)
        }
    }

    /// Rewrites the file. If the old file contains comments, it is kept once as `config.toml.bak`, because comments are
    /// lost when rewriting.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Comments are lost when writing: a commented file is backed up with a timestamp first.
        if let Ok(old) = std::fs::read_to_string(path)
            && old.lines().any(|l| l.trim_start().starts_with('#'))
        {
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            let backup = path.with_extension(format!("toml.{stamp}.bak"));
            std::fs::copy(path, &backup).with_context(|| (t().backup_unwritable)(&backup.display().to_string()))?;
        }
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(path, raw).with_context(|| (t().config_unwritable)(&path.display().to_string()))
    }

    pub fn archive_root(&self) -> PathBuf {
        PathBuf::from(&self.archive_root)
    }
}

/// Time of the last change of a file (for "was it changed outside in the meantime?").
pub fn modified(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_ocr_settings_still_load() {
        let old = "[ocr]\nengine = \"ocrs\"\ntesseract_path = \"tesseract\"\ntesseract_lang = \"deu+fra\"\n";
        let cfg: Config = toml::from_str(old).unwrap();
        assert_eq!(cfg.ocr.languages, "deu+fra");
        assert_eq!(cfg.ocr.quality, OcrQuality::Fast);
        let again: Config = toml::from_str(&toml::to_string_pretty(&cfg).unwrap()).unwrap();
        assert_eq!(again.ocr, cfg.ocr);
        let mut ocr = OcrConfig { languages: "deu + eng+deu, ita".into(), ..OcrConfig::default() };
        assert_eq!(ocr.language_list(), ["deu", "eng", "ita"]);
        ocr.set_languages(&["eng".into(), "spa".into()]);
        assert_eq!(ocr.languages, "eng+spa");
    }

    #[test]
    fn roundtrip_and_partial_files() {
        let mut cfg = Config::default();
        cfg.rules.push(Rule {
            keywords: vec!["Stadtwerke".into()],
            folder: "Wohnung/{year}".into(),
            doctype: Some("Rechnung".into()),
            ..Default::default()
        });
        cfg.rules.push(Rule { name: "Aus".into(), enabled: false, match_all: true, ..Default::default() });
        let raw = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&raw).unwrap();
        assert_eq!(cfg, back);

        let partial: Config = toml::from_str("archive_root = \"/tmp/x\"\n[llm]\nmodel = \"m\"\n").unwrap();
        assert_eq!(partial.archive_root, "/tmp/x");
        assert_eq!(partial.llm.model, "m");
        assert_eq!(partial.llm.base_url, LlmConfig::default().base_url);
        assert_eq!(partial.llm.backend, LlmBackend::Embedded);
        assert_eq!(partial.llm.gpu_layers, -1);

        let server: Config = toml::from_str("[llm]\nbackend = \"server\"\nbase_url = \"http://x/v1\"\n").unwrap();
        assert_eq!(server.llm.backend, LlmBackend::Server);
        assert_eq!(server.llm.context_size, 12288);
    }

    #[test]
    fn example_config_parses() {
        let raw = include_str!("../config.example.toml");
        let cfg: Config = toml::from_str(raw).unwrap();
        assert!(!cfg.rules.is_empty());
        assert!(cfg.rules.iter().all(|r| r.enabled), "older rules without enabled are active");
        assert_eq!(cfg.folder_words.get("Versicherungen/Haftpflicht").map(String::as_str), Some("Haftpflicht*"));
    }
}
