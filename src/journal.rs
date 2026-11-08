//! Decision log: what Scrinium suggested and what was filed.
//!
//! One JSON line per filing and per undo in `decisions.jsonl` next to the configuration. It lets you check
//! keywords, folder descriptions and rules against real decisions; `scrinium --report` summarises the file.
//! The recognised text is not stored – the document can be read again via the target path.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::analysis::{self, Analysis, Origin};
use crate::config::{self, Config};
use crate::form::{self, Form};

/// Name of the file next to `config.toml`.
const FILE: &str = "decisions.jsonl";

/// Path of the log; with `SCRINIUM_CONFIG` it lies next to the chosen configuration.
pub fn path() -> PathBuf {
    config::config_path().parent().unwrap_or(Path::new(".")).join(FILE)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum Entry {
    Filed(Box<Filed>),
    Undone(Undone),
}

/// A filed document: suggestion and filed value per field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Filed {
    /// Time (RFC 3339, local time).
    pub time: String,
    pub version: String,
    /// Original file name.
    pub file: String,
    /// Where the file is now.
    pub target: String,
    /// Where the suggestion comes from, e.g. „KI (Qwen3-VL 8B, …)“ or „Heuristik“.
    pub source: String,
    /// Applied special rule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// The AI's confidence; missing without AI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    /// Duration of the analysis in seconds.
    pub seconds: f32,
    pub folder: Choice,
    /// Reasons for each stage of the folder choice.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub folder_why: Vec<String>,
    /// Notes on the folder, e.g. “year folder for 2026”.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub folder_notes: Vec<String>,
    pub filename: Choice,
    /// File name written by hand instead of built from the parts.
    pub filename_manual: bool,
    pub fields: Fields,
    /// Parts that were turned off in the file name.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub off: Vec<String>,
}

/// A filing was undone; it no longer counts in the report.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Undone {
    pub time: String,
    pub target: String,
}

/// Suggested and filed value of a field, plus the origin of the suggestion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Choice {
    pub suggested: String,
    #[serde(rename = "final")]
    pub chosen: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub origin: String,
}

impl Choice {
    fn new(origin: Origin, suggested: &str, chosen: &str) -> Self {
        Self { suggested: suggested.into(), chosen: chosen.into(), origin: origin.key().into() }
    }

    pub fn changed(&self) -> bool {
        self.suggested.trim() != self.chosen.trim()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Fields {
    pub date: Choice,
    pub doctype: Choice,
    pub sender: Choice,
    pub subject: Choice,
    pub person: Choice,
    pub extra: Choice,
}

impl Filed {
    /// Entry from the analysis and the confirmed form.
    pub fn new(cfg: &Config, a: &Analysis, form: &Form, source: &Path, target: &Path) -> Self {
        let s = &a.suggestion;
        let o = &s.origin;
        // The form as it was after the analysis, with the file name built from it.
        let proposed = Form::from_analysis(a);
        let proposed_name = form::auto_filename(cfg, &proposed, s.keep_name, &analysis::original_stem(source));
        Self {
            time: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            version: env!("CARGO_PKG_VERSION").into(),
            file: source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            target: target.display().to_string(),
            source: s.source.clone(),
            rule: s.rule.clone(),
            confidence: s.confidence_from_ai.then_some(s.confidence),
            seconds: (a.duration.as_secs_f32() * 10.0).round() / 10.0,
            folder: Choice::new(o.folder, &proposed.folder, &form.folder),
            folder_why: s.folder_why.clone(),
            folder_notes: s.folder_notes.clone(),
            filename: Choice::new(Origin::None, &proposed_name, &form.filename),
            filename_manual: form.filename_manual,
            fields: Fields {
                date: Choice::new(o.date, &proposed.date, &form.date),
                doctype: Choice::new(o.doctype, &proposed.doctype, &form.doctype),
                sender: Choice::new(o.sender, &proposed.sender, &form.sender),
                subject: Choice::new(o.subject, &proposed.subject, &form.subject),
                person: Choice::new(o.person, &proposed.person, &form.person),
                extra: Choice::new(o.extra, &proposed.extra, &form.extra),
            },
            off: form.off_fields(),
        }
    }

    /// Folder, file name and the six fields with their labels, for the report.
    fn choices(&self) -> [(&'static str, &Choice); 8] {
        let f = &self.fields;
        [
            ("Folder", &self.folder),
            ("File name", &self.filename),
            ("Date", &f.date),
            ("Document type", &f.doctype),
            ("Sender", &f.sender),
            ("Subject", &f.subject),
            ("Person", &f.person),
            ("Extra", &f.extra),
        ]
    }
}

impl Undone {
    pub fn new(target: &Path) -> Self {
        Self {
            time: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            target: target.display().to_string(),
        }
    }
}

/// Appends an entry as one line.
pub fn append(path: &Path, entry: &Entry) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut line = serde_json::to_string(entry)?;
    line.push('\n');
    OpenOptions::new().create(true).append(true).open(path)?.write_all(line.as_bytes())?;
    Ok(())
}

/// Reads the log, plus the number of lines that could not be read.
pub fn read(path: &Path) -> Result<(Vec<Entry>, usize)> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("{} is not readable", path.display()))?;
    let mut entries = Vec::new();
    let mut broken = 0;
    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str(line) {
            Ok(e) => entries.push(e),
            Err(_) => broken += 1,
        }
    }
    Ok((entries, broken))
}

/// Order of the origins in the report; empty = no suggestion.
const ORIGINS: [&str; 8] = ["ai", "words", "rule", "text", "filename", "folder", "fallback", ""];

fn origin_label(key: &str) -> &str {
    match key {
        "ai" => "AI",
        "words" => "Keyword",
        "rule" => "Rule",
        "text" => "Text",
        "filename" => "File name",
        "folder" => "Folder",
        "fallback" => "Fallback folder",
        "" => "empty",
        other => other,
    }
}

fn shown(value: &str) -> &str {
    if value.trim().is_empty() { "(empty)" } else { value }
}

/// Day of a point in time as `2026-10-13`.
fn day(time: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(time).map_or_else(|_| time.to_string(), |t| t.format("%Y-%m-%d").to_string())
}

/// Summary for `scrinium --report`: how often the suggestion was kept per field, by origin, and which
/// changes recur. Undone filings do not count.
pub fn report(entries: &[Entry]) -> String {
    let mut filed: Vec<&Filed> = Vec::new();
    let mut undone = 0;
    for e in entries {
        match e {
            Entry::Filed(f) => filed.push(f),
            Entry::Undone(u) => {
                if let Some(i) = filed.iter().rposition(|f| f.target == u.target) {
                    filed.remove(i);
                    undone += 1;
                }
            }
        }
    }
    let (Some(first), Some(last)) = (filed.first(), filed.last()) else {
        return "No decisions recorded yet.\n".into();
    };
    let total = filed.len();
    let docs = if total == 1 { "document" } else { "documents" };
    let mut out = format!("{total} {docs} filed, {} to {}", day(&first.time), day(&last.time));
    match undone {
        0 => {}
        1 => out.push_str(" (1 undone filing not counted)"),
        n => {
            let _ = write!(out, " ({n} undone filings not counted)");
        }
    }
    out.push_str("\n\nSuggestion kept\n");
    for i in 0..8 {
        let label = first.choices()[i].0;
        let choices: Vec<&Choice> = filed.iter().map(|f| f.choices()[i].1).collect();
        let kept = choices.iter().filter(|c| !c.changed()).count();
        let percent = (kept * 100 + total / 2) / total;
        let _ = write!(out, "  {label:<14}{kept:>4} of {total:<4}{percent:>4} %");
        // By origin only if there is one (the file name has none of its own).
        if choices.iter().any(|c| !c.origin.is_empty()) {
            let parts: Vec<String> = ORIGINS
                .iter()
                .filter_map(|&key| {
                    let of: Vec<_> = choices.iter().filter(|c| c.origin == key).collect();
                    let kept = of.iter().filter(|c| !c.changed()).count();
                    (!of.is_empty()).then(|| format!("{} {kept}/{}", origin_label(key), of.len()))
                })
                .collect();
            let _ = write!(out, "   {}", parts.join(" · "));
        }
        out.push('\n');
    }

    let off: Vec<String> = ["date", "doctype", "sender", "subject", "person", "extra"]
        .iter()
        .zip(["Date", "Document type", "Sender", "Subject", "Person", "Extra"])
        .filter_map(|(key, label)| {
            let n = filed.iter().filter(|f| f.off.iter().any(|o| o == key)).count();
            (n > 0).then(|| format!("{label} {n}×"))
        })
        .collect();
    if !off.is_empty() {
        let _ = writeln!(out, "\nTurned off in the file name: {}", off.join(", "));
    }

    for (i, limit) in [(0, 15), (3, 10), (4, 10), (6, 10)] {
        corrections(&mut out, &filed, i, limit);
    }
    out
}

/// Recurring changes to a field: suggested → filed, with the origin of the suggestion.
fn corrections(out: &mut String, filed: &[&Filed], i: usize, limit: usize) {
    let mut pairs: HashMap<(&str, &str), (usize, Vec<&str>)> = HashMap::new();
    for f in filed {
        let c = f.choices()[i].1;
        if c.changed() {
            let entry = pairs.entry((c.suggested.trim(), c.chosen.trim())).or_default();
            entry.0 += 1;
            if !entry.1.contains(&c.origin.as_str()) {
                entry.1.push(&c.origin);
            }
        }
    }
    if pairs.is_empty() {
        return;
    }
    let mut pairs: Vec<_> = pairs.into_iter().collect();
    pairs.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(&b.0)));
    let label = filed[0].choices()[i].0;
    let _ = writeln!(out, "\n{label} changed (suggested → filed)");
    for ((suggested, chosen), (n, origins)) in pairs.iter().take(limit) {
        let origins: Vec<&str> = origins.iter().map(|o| origin_label(o)).collect();
        let _ = writeln!(out, "  {n:>3}×  {} → {}   [{}]", shown(suggested), shown(chosen), origins.join(", "));
    }
    if pairs.len() > limit {
        let _ = writeln!(out, "  … {} more", pairs.len() - limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filed(target: &str, folder: (&str, &str, &str), doctype: (&str, &str, &str)) -> Entry {
        let c = |(o, s, f): (&str, &str, &str)| Choice { suggested: s.into(), chosen: f.into(), origin: o.into() };
        Entry::Filed(Box::new(Filed {
            time: "2026-10-13T10:00:00+02:00".into(),
            target: target.into(),
            folder: c(folder),
            filename: c(("", "a", "a")),
            fields: Fields { doctype: c(doctype), ..Default::default() },
            ..Default::default()
        }))
    }

    #[test]
    fn entry_from_analysis_and_form() {
        let source = Path::new("/eingang/scan_001.pdf");
        let suggestion = analysis::Suggestion {
            date: crate::naming::DocDate::parse("01.10.2026"),
            doctype: "Brief".into(),
            sender: "Stadtwerke".into(),
            folder: "Wohnen/Strom".into(),
            source: "AI (Test)".into(),
            confidence: 0.8,
            confidence_from_ai: true,
            origin: analysis::Origins { doctype: Origin::Ai, folder: Origin::Words, ..Default::default() },
            ..Default::default()
        };
        let a = Analysis {
            suggestion,
            kind: crate::ingest::DocKind::of(source),
            text: String::new(),
            previews: Vec::new(),
            page_count: 1,
            ocr_engine: None,
            corrections: Vec::new(),
            rotations: Vec::new(),
            warnings: Vec::new(),
            duration: std::time::Duration::from_millis(12_340),
            ai_stats: None,
        };
        let cfg = Config::default();
        let mut form = Form::from_analysis(&a);
        form.doctype = "Rechnung".into();
        form.off = 1; // date in the name off
        form.filename = form::auto_filename(&cfg, &form, false, "scan_001");
        let f = Filed::new(&cfg, &a, &form, source, Path::new("/ablage/Wohnen/Strom/x.pdf"));
        assert_eq!(f.file, "scan_001.pdf");
        assert_eq!((f.confidence, f.seconds), (Some(0.8), 12.3));
        assert!(!f.folder.changed() && f.folder.origin == "words");
        assert_eq!((f.fields.doctype.suggested.as_str(), f.fields.doctype.chosen.as_str()), ("Brief", "Rechnung"));
        assert_eq!(f.fields.doctype.origin, "ai");
        assert!(!f.fields.date.changed() && f.fields.date.origin.is_empty());
        assert!(f.filename.changed(), "{} → {}", f.filename.suggested, f.filename.chosen);
        assert_eq!(f.off, ["date"]);
    }

    #[test]
    fn lines_round_trip() {
        let dir = std::env::temp_dir().join(format!("scrinium-journal-{}", std::process::id()));
        let path = dir.join(FILE);
        let _ = std::fs::remove_file(&path);
        append(&path, &filed("/a/x.pdf", ("words", "Bank", "Bank"), ("ai", "Brief", "Rechnung"))).unwrap();
        append(&path, &Entry::Undone(Undone { time: String::new(), target: "/a/x.pdf".into() })).unwrap();
        std::fs::write(&path, std::fs::read_to_string(&path).unwrap() + "kaputt\n").unwrap();
        let (entries, broken) = read(&path).unwrap();
        assert_eq!((entries.len(), broken), (2, 1));
        let Entry::Filed(f) = &entries[0] else { panic!("filed first") };
        assert_eq!((f.folder.origin.as_str(), f.fields.doctype.chosen.as_str()), ("words", "Rechnung"));
        assert!(f.fields.doctype.changed() && !f.folder.changed());
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.starts_with(r#"{"action":"filed","#), "{raw}");
        assert!(raw.contains(r#""final":"Rechnung""#), "{raw}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn report_counts_and_corrections() {
        let entries = vec![
            filed("/a/1.pdf", ("words", "Bank", "Bank"), ("ai", "Brief", "Rechnung")),
            filed("/a/2.pdf", ("ai", "Versicherung/Haftpflicht", "Versicherung/Hausrat"), ("ai", "Brief", "Rechnung")),
            filed("/a/3.pdf", ("ai", "Steuer", "Steuer"), ("ai", "Bescheid", "Bescheid")),
            // filed, undone and filed again differently: counts once
            filed("/a/4.pdf", ("ai", "Bank", "Bank"), ("ai", "Brief", "Brief")),
            Entry::Undone(Undone { time: String::new(), target: "/a/4.pdf".into() }),
            filed("/a/4b.pdf", ("ai", "Bank", "Gehalt"), ("ai", "Brief", "Brief")),
        ];
        let r = report(&entries);
        assert!(r.starts_with("4 documents filed, 2026-10-13 to 2026-10-13 (1 undone filing "), "{r}");
        assert!(r.contains("Folder           2 of 4     50 %   AI 1/3 · Keyword 1/1"), "{r}");
        assert!(r.contains("Document type    2 of 4     50 %   AI 2/4"), "{r}");
        assert!(r.contains("File name        4 of 4    100 %\n"), "{r}");
        assert!(r.contains("    2×  Brief → Rechnung   [AI]"), "{r}");
        assert!(r.contains("    1×  Versicherung/Haftpflicht → Versicherung/Hausrat   [AI]"), "{r}");
        assert!(!r.contains("Sender changed"), "{r}");
    }

    #[test]
    fn empty_report() {
        assert_eq!(report(&[]), "No decisions recorded yet.\n");
    }
}
