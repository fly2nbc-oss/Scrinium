//! Setup with AI: the local AI reads a description of the filing and file naming system in the user's own words
//! – or existing documentation – and proposes the settings from it: file name template, document types, persons,
//! short names, folder descriptions, special rules and instructions for the AI. Nothing is applied until the user
//! has reviewed it, section by section.

use std::fmt::Write as _;

use eframe::egui::{self, RichText, ScrollArea, TextEdit};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::archive::{self, FolderIndex};
use crate::config::{Alias, Config, Rule};
use crate::theme::{self, text, tokens};
use tokens::{muted, status_err, status_ok, status_warn};

/// Longest AI answer in tokens; enough for 20 rules and 40 folder notes including lists.
pub const MAX_ANSWER_TOKENS: usize = 9000;
/// Largest context for the request (description, folder list and answer).
pub const MAX_CONTEXT: u32 = 32768;

/// The AI's proposal, field by field as in the response schema.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Proposal {
    pub filename_template: String,
    pub date_format: String,
    pub fallback_folder: String,
    pub doctypes: Vec<String>,
    pub extras: Vec<String>,
    pub persons: Vec<Alias>,
    pub person_from_folder: bool,
    pub senders: Vec<Alias>,
    pub folder_notes: Vec<FolderNote>,
    pub rules: Vec<ProposedRule>,
    pub instructions: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct FolderNote {
    pub folder: String,
    pub note: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ProposedRule {
    pub name: String,
    pub keywords: Vec<String>,
    pub match_all: bool,
    pub require_keywords: Vec<String>,
    pub exclude_keywords: Vec<String>,
    pub folder: String,
    pub doctype: String,
    pub person: String,
}

/// Response schema. Folders are deliberately not a fixed choice in the schema: the grammar would become large and
/// the long answer slow. They are checked against the archive afterwards (`apply`).
pub fn schema() -> Value {
    let strings = |max: usize| json!({ "type": "array", "items": { "type": "string" }, "maxItems": max });
    let alias = json!({
        "type": "object",
        "properties": { "name": { "type": "string" }, "aliases": strings(8) },
        "required": ["name", "aliases"],
        "additionalProperties": false
    });
    json!({
        "type": "object",
        "properties": {
            "filename_template": { "type": "string" },
            "date_format": { "type": "string" },
            "fallback_folder": { "type": "string" },
            "doctypes": strings(60),
            "extras": strings(20),
            "persons": { "type": "array", "items": alias, "maxItems": 12 },
            "person_from_folder": { "type": "boolean" },
            "senders": { "type": "array", "items": alias, "maxItems": 25 },
            "folder_notes": {
                "type": "array",
                "maxItems": 40,
                "items": {
                    "type": "object",
                    "properties": { "folder": { "type": "string" }, "note": { "type": "string" } },
                    "required": ["folder", "note"],
                    "additionalProperties": false
                }
            },
            "rules": {
                "type": "array",
                "maxItems": 20,
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "keywords": strings(12),
                        "match_all": { "type": "boolean" },
                        "require_keywords": strings(6),
                        "exclude_keywords": strings(12),
                        "folder": { "type": "string" },
                        "doctype": { "type": "string" },
                        "person": { "type": "string" }
                    },
                    "required": ["name", "keywords", "match_all", "require_keywords", "exclude_keywords", "folder", "doctype", "person"],
                    "additionalProperties": false
                }
            },
            "instructions": { "type": "string" }
        },
        "required": [
            "filename_template", "date_format", "fallback_folder", "doctypes", "extras", "persons",
            "person_from_folder", "senders", "folder_notes", "rules", "instructions"
        ],
        "additionalProperties": false
    })
}

pub fn system_prompt() -> String {
    "Du richtest Scrinium ein, eine App, die Dokumente benennt und in eine vorhandene Ordnerablage \
     einsortiert. Du bekommst die vorhandenen Ordner und eine Beschreibung des Ablage- und \
     Dateinamen-Systems. Leite daraus die Einstellungen ab. Übernimm nur, was die Beschreibung \
     hergibt; erfinde keine Ordner, Personen, Nummern oder Regeln. Antworte nur mit JSON:\n\
     - filename_template: Vorlage aus den Bausteinen {date} {doctype} {sender} {subject} {person} {extra}, \
       mit _ getrennt, in der beschriebenen Reihenfolge, z. B. {date}_{doctype}_{sender}_{subject}_{person}_{extra}.\n\
     - date_format: Format für {date}: JJMMTT = %y%m%d, JJJJ-MM-TT = %Y-%m-%d, JJJJMMTT = %Y%m%d.\n\
     - fallback_folder: Auffangordner aus der Liste für Dokumente, die nirgends passen, sonst leer.\n\
     - doctypes: die erlaubten Dokumentarten genau wie beschrieben, in der Einzahl; leer, wenn keine feste Liste beschrieben ist.\n\
     - extras: erlaubte Zusätze am Ende des Namens (z. B. unterschrieben, Kopie).\n\
     - persons: Personen mit dem Namen, der in Dateinamen steht (name), und anderen Schreibweisen (aliases: volle Namen, Kürzel).\n\
     - person_from_folder: true, wenn die Person nur bei persönlichen Dokumenten im Namen steht (Gehalt, Gesundheit, Zeugnis, Urkunde), die in Ordnern der Person liegen.\n\
     - senders: Kurznamen für Absender (name) mit ihren langen Schreibweisen (aliases).\n\
     - folder_notes: für die höchstens 40 wichtigsten Ordner aus der Liste, was hineingehört – höchstens 12 Wörter, \
       Ordnerpfad genau wie in der Liste. Danach wählt die KI später den Ordner, Ebene für Ebene: Beschreibe vor \
       allem die oberste Ebene und nimm hier auf, was die Beschreibung über die Zuordnung sagt (z. B. „Alle \
       Schreiben von Versicherungen“, „Urkunden auch vom Sport, je Person“).\n\
     - rules: eindeutige Zuordnungen, die immer gelten, z. B. eine Kontonummer, Personalnummer, Vertragsnummer \
       oder ein Absender, der immer in denselben Ordner gehört. keywords: Wörter, die im Dokument stehen; \
       require_keywords: müssen zusätzlich vorkommen; exclude_keywords: dann gilt die Regel nicht; \
       match_all: alle keywords nötig; folder: Ordner aus der Liste, {year} für Jahresordner (z. B. Bank/{year}); \
       doctype und person nur, wenn die Regel sie festlegt, sonst leer. Höchstens 20 Regeln, die wichtigste \
       zuerst; was die KI am Thema erkennt, braucht keine Regel.\n\
     - instructions: höchstens 8 kurze Zeilen mit „- “: wie die Angaben im Dateinamen aussehen (Absender und \
       Betreff je Dokumentart, z. B. „Kontoauszug: Betreff ist der Monat JJJJ-MM“). Was in einen Ordner gehört, \
       steht in folder_notes, nicht hier.\n\
     Fasse dich kurz und lass weg, was die Beschreibung nicht hergibt."
        .to_string()
}

pub fn user_prompt(description: &str, folders: &[String]) -> String {
    let mut s = String::from("Vorhandene Ordner:\n");
    for f in folders {
        let _ = writeln!(s, "- {f}");
    }
    let _ = write!(s, "\nBeschreibung des Ablagesystems:\n{}", description.trim());
    s
}

/// Sections to apply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sections {
    pub naming: bool,
    pub doctypes: bool,
    pub persons: bool,
    pub senders: bool,
    pub notes: bool,
    pub rules: bool,
    pub instructions: bool,
    /// Only folders without a description get one; existing ones stay.
    pub only_empty_notes: bool,
}

impl Default for Sections {
    fn default() -> Self {
        Self {
            naming: true,
            doctypes: true,
            persons: true,
            senders: true,
            notes: true,
            rules: true,
            instructions: true,
            only_empty_notes: true,
        }
    }
}

/// How many folder descriptions of the proposal would be new, would replace existing ones, and have no folder.
fn note_counts(p: &Proposal, cfg: &Config, folders: &FolderIndex) -> (usize, usize, usize) {
    let (mut new, mut existing, mut missing) = (0, 0, 0);
    for n in p.folder_notes.iter().filter(|n| !n.note.trim().is_empty()) {
        match archive::find_existing_folder(&folders.all, n.folder.trim()) {
            Some(f) if cfg.folder_notes.get(f).is_some_and(|v| !v.trim().is_empty()) => existing += 1,
            Some(_) => new += 1,
            None => missing += 1,
        }
    }
    (new, existing, missing)
}

fn clean_list(v: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in v.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if !out.iter().any(|o| o.eq_ignore_ascii_case(s)) {
            out.push(s.to_string());
        }
    }
    out
}

fn clean_aliases(v: &[Alias]) -> Vec<Alias> {
    let mut out: Vec<Alias> = Vec::new();
    for a in v.iter().filter(|a| !a.name.trim().is_empty()) {
        if out.iter().any(|o| o.name.eq_ignore_ascii_case(a.name.trim())) {
            continue;
        }
        let name = a.name.trim().to_string();
        let aliases = clean_list(&a.aliases).into_iter().filter(|x| !x.eq_ignore_ascii_case(&name)).collect();
        out.push(Alias { name, aliases });
    }
    out
}

/// Valid date format? (chrono reports errors only when formatting.)
fn valid_date_format(fmt: &str) -> bool {
    let Some(date) = chrono::NaiveDate::from_ymd_opt(2026, 9, 25) else {
        return false;
    };
    let mut s = String::new();
    !fmt.trim().is_empty() && write!(s, "{}", date.format(fmt)).is_ok()
}

/// A rule's folder without placeholders: `Bank/{year}` → `Bank`.
fn fixed_part(folder: &str) -> String {
    archive::split_folder(folder).into_iter().take_while(|p| !p.contains('{')).collect::<Vec<_>>().join("/")
}

/// Applies the selected sections to `cfg`; returns notes on what was adjusted or left out.
pub fn apply(p: &Proposal, cfg: &mut Config, folders: &FolderIndex, take: Sections) -> Vec<String> {
    let t = crate::i18n::setup::t();
    let mut notes = Vec::new();
    if take.naming {
        let template = p.filename_template.trim();
        let known = [
            "{date}",
            "{doctype}",
            "{sender}",
            "{subject}",
            "{person}",
            "{extra}",
            "{year}",
            "{month}",
            "{day}",
            "{title}",
        ];
        let unknown: Vec<&str> = template
            .match_indices('{')
            .filter_map(|(i, _)| template[i..].find('}').map(|e| &template[i..=i + e]))
            .filter(|p| !known.contains(p))
            .collect();
        if template.contains('{') && unknown.is_empty() {
            cfg.filename_template = template.to_string();
        } else if !template.is_empty() {
            notes.push((t.template_rejected)(template, &unknown.join(" ")));
        }
        if valid_date_format(&p.date_format) {
            cfg.date_format = p.date_format.trim().to_string();
        } else if !p.date_format.trim().is_empty() {
            notes.push((t.date_format_invalid)(&p.date_format, &cfg.date_format));
        }
        let fallback = p.fallback_folder.trim();
        if let Some(f) = archive::find_existing_folder(&folders.all, fallback) {
            cfg.fallback_folder = f.clone();
        } else if !fallback.is_empty() {
            notes.push((t.fallback_missing)(fallback, &cfg.fallback_folder));
        }
    }
    if take.doctypes {
        let doctypes = clean_list(&p.doctypes);
        if doctypes.len() >= 3 {
            cfg.doctypes = doctypes;
        }
        let extras = clean_list(&p.extras);
        if !extras.is_empty() {
            cfg.extras = extras;
        }
    }
    if take.persons {
        let persons = clean_aliases(&p.persons);
        if !persons.is_empty() {
            cfg.persons = persons;
        }
        cfg.person_from_folder = p.person_from_folder;
    }
    if take.senders {
        for s in clean_aliases(&p.senders) {
            match cfg.senders.iter_mut().find(|o| o.name.eq_ignore_ascii_case(&s.name)) {
                Some(o) => o.aliases = clean_list(&[o.aliases.clone(), s.aliases].concat()),
                None => cfg.senders.push(s),
            }
        }
    }
    if take.notes {
        let (mut skipped, mut kept) = (0, 0);
        for n in p.folder_notes.iter().filter(|n| !n.note.trim().is_empty()) {
            match archive::find_existing_folder(&folders.all, n.folder.trim()) {
                Some(f) if take.only_empty_notes && cfg.folder_notes.get(f).is_some_and(|v| !v.trim().is_empty()) => {
                    kept += 1;
                }
                Some(f) => {
                    cfg.folder_notes.insert(f.clone(), n.note.trim().to_string());
                }
                None => skipped += 1,
            }
        }
        if kept > 0 {
            notes.push((t.notes_kept)(kept));
        }
        if skipped > 0 {
            notes.push((t.notes_skipped)(skipped));
        }
    }
    if take.rules && !p.rules.is_empty() {
        let mut rules = Vec::new();
        for r in &p.rules {
            let keywords = clean_list(&r.keywords);
            let folder = r.folder.trim().replace('\\', "/");
            if keywords.is_empty() || folder.is_empty() || archive::safe_relative_folder(&folder).is_err() {
                continue;
            }
            let fixed = fixed_part(&folder);
            let exists = fixed.is_empty() || archive::find_existing_folder(&folders.all, &fixed).is_some();
            let name = if r.name.trim().is_empty() { keywords[0].clone() } else { r.name.trim().to_string() };
            if !exists {
                notes.push((t.rule_folder_missing)(&name, &fixed));
            }
            let opt = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
            rules.push(Rule {
                name,
                enabled: exists,
                keywords,
                match_all: r.match_all,
                exclude_keywords: clean_list(&r.exclude_keywords),
                require_keywords: clean_list(&r.require_keywords),
                folder,
                doctype: opt(&r.doctype),
                sender: None,
                person: opt(&r.person),
            });
        }
        if !rules.is_empty() {
            if !cfg.rules.is_empty() {
                notes.push((t.rules_replaced)(cfg.rules.len()));
            }
            cfg.rules = rules;
        }
    }
    if take.instructions && !p.instructions.trim().is_empty() {
        cfg.llm.instructions = p.instructions.trim().to_string();
    }
    notes
}

/// State of the *Setup with AI* tab; kept when the settings are closed.
#[derive(Default)]
pub struct Panel {
    pub description: String,
    pub running: bool,
    /// Cancels the running job.
    pub cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub status: String,
    pub error: Option<String>,
    pub proposal: Option<Proposal>,
    pub take: Sections,
    /// Notes after applying.
    pub applied: Option<Vec<String>>,
}

pub enum PanelAction {
    Generate,
    Apply,
}

const EXAMPLE: &str = "\
Meine Ablage ist nach Bereichen gegliedert: 01_Versicherung, 02_Wohnung, 03_Auto, 04_Finanzen, \
05_Arbeit, 06_Haushalt, 07_Familie, 08_Steuern. Unklares kommt nach 00_Eingang.

Dateinamen: JJMMTT_Dokumentart_Absender_Betreff_Person, z. B. 260925_Steuerbescheid_Finanzamt_2025. \
Ist der Tag unbekannt, steht 00. Leerzeichen werden zu Bindestrichen.

Dokumentarten: Rechnung, Beitragsrechnung, Kontoauszug, Vertrag, Kündigung, Bescheid, Steuerbescheid, \
Gehaltsabrechnung, Zeugnis, Urkunde, Arztbrief, Brief (eigenes Schreiben), Schreiben, Mitteilung.

Personen: Max (Maximilian Muster, MM) und Erika (Erika Muster). Die Person steht nur bei persönlichen \
Dokumenten wie Gehalt, Arzt oder Zeugnissen im Namen.

Kurznamen: Stadtwerke Musterstadt → SWM, Exempla Versicherungs-AG → Exempla.

Gehaltsabrechnungen von Max (Personalnummer 12345) kommen nach 05_Arbeit/Gehalt/JJJJ_Max. \
Kontoauszüge der Sparkasse (Konto 1234567) nach 04_Finanzen/Sparkasse/JJJJ. \
Schreiben einer Versicherung gehören immer nach 01_Versicherung, auch wenn es um das Auto geht.";

/// List with its own scroll bar: the whole proposal stays visible without lengthening the page.
fn scroll_list(ui: &mut egui::Ui, id: &str, lines: impl Iterator<Item = String>) {
    ScrollArea::vertical().id_salt(("setup-list", id)).max_height(180.0).show(ui, |ui| {
        for line in lines {
            ui.label(RichText::new(line).size(text::SMALL));
        }
    });
}

fn section(ui: &mut egui::Ui, on: &mut bool, title: &str, effect: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        theme::checkbox(ui, on, RichText::new(title).size(text::BODY).color(if *on { tokens::fg() } else { muted() }));
        if !effect.is_empty() {
            ui.label(RichText::new(effect).small().color(if *on { status_warn() } else { muted() }));
        }
    });
    let off = crate::i18n::setup::t().section_off;
    ui.indent(title, |ui| {
        ui.add_enabled_ui(*on, body).response.on_disabled_hover_text(off);
    });
}

/// The *Setup with AI* tab. `current`: the draft as it would be saved (for "new" and "replaced").
/// `ai_enabled`: the AI is switched on in the draft – that is enough, the job loads the model itself.
pub fn panel_ui(
    ui: &mut egui::Ui,
    panel: &mut Panel,
    current: &Config,
    folders: &FolderIndex,
    ai: &crate::settings::AiStatus,
    ai_enabled: bool,
) -> Option<PanelAction> {
    let t = crate::i18n::setup::t();
    let mut action = None;
    ui.label(RichText::new(t.intro).size(text::BODY));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.add(theme::secondary_button(t.load_file)).clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter(t.file_filter, &["md", "markdown", "txt"])
                .set_title(t.load_title)
                .pick_file()
        {
            match std::fs::read(&path) {
                Ok(bytes) => panel.description = crate::ingest::decode_text(&bytes),
                Err(e) => panel.error = Some(format!("{}: {e}", path.display())),
            }
        }
        if theme::ghost_button(ui, t.insert_example).clicked() {
            panel.description = EXAMPLE.to_string();
        }
        if !panel.description.is_empty() && theme::ghost_button(ui, t.clear).clicked() {
            panel.description.clear();
        }
        ui.label(RichText::new((t.chars)(panel.description.chars().count())).small().color(muted()));
    });
    theme::text_area(
        ui,
        TextEdit::multiline(&mut panel.description)
            .hint_text(t.description_hint)
            .desired_rows(12)
            .desired_width(f32::INFINITY),
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let can = !panel.running && !panel.description.trim().is_empty() && ai_enabled;
        let blocked = if panel.running {
            t.generate_running
        } else if !ai_enabled {
            t.generate_needs_ai
        } else {
            t.generate_needs_text
        };
        let resp = ui
            .add_enabled(can, theme::primary_button(theme::with_icon(theme::icon::SPARKLES, t.generate)))
            .on_disabled_hover_text(blocked);
        if resp.clicked() {
            action = Some(PanelAction::Generate);
        }
        if panel.running {
            ui.spinner();
            ui.label(RichText::new(&panel.status).color(muted()));
            if ui.add(theme::secondary_button(t.cancel)).on_hover_text(t.cancel_hint).clicked() {
                if let Some(c) = panel.cancel.take() {
                    c.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                panel.running = false;
                panel.status.clear();
            }
        } else if !ai_enabled {
            ui.label(RichText::new(t.ai_off).small().color(status_warn()));
        } else if !ai.ready {
            ui.label(RichText::new((t.ai_status)(&ai.text)).small().color(muted()));
        }
    });
    if let Some(e) = &panel.error {
        ui.label(RichText::new(e).color(status_err()));
    }
    if let Some(notes) = &panel.applied {
        ui.add_space(8.0);
        ui.label(RichText::new(t.applied).color(status_ok()));
        for n in notes {
            ui.label(RichText::new(format!("• {n}")).small().color(status_warn()));
        }
    }
    let Some(p) = panel.proposal.as_ref() else { return action };
    ui.add_space(12.0);
    ui.separator();
    ui.label(RichText::new(t.proposal).size(text::LARGE));
    hint_line(ui, t.proposal_hint);
    let take = &mut panel.take;
    section(ui, &mut take.naming, t.naming, "", |ui| {
        ui.label(RichText::new(&p.filename_template).monospace());
        ui.label((t.naming_summary)(&p.date_format, &p.fallback_folder));
    });
    let replaces = |n: usize| if n > 0 { (t.replaces_existing)(n) } else { String::new() };
    section(
        ui,
        &mut take.doctypes,
        &(t.doctypes)(p.doctypes.len(), p.extras.len()),
        &replaces(current.doctypes.len()),
        |ui| {
            ui.label(RichText::new(p.doctypes.join(", ")).size(text::SMALL));
            if !p.extras.is_empty() {
                ui.label(RichText::new((t.extras)(&p.extras.join(", "))).size(text::SMALL).color(muted()));
            }
        },
    );
    section(ui, &mut take.persons, &(t.persons)(p.persons.len()), &replaces(current.persons.len()), |ui| {
        scroll_list(ui, "persons", p.persons.iter().map(|a| format!("{} ← {}", a.name, a.aliases.join(", "))));
        if p.person_from_folder {
            ui.label(RichText::new(t.person_from_folder).small().color(muted()));
        }
    });
    let known =
        p.senders.iter().filter(|s| current.senders.iter().any(|o| o.name.eq_ignore_ascii_case(&s.name))).count();
    let effect = if known > 0 { (t.senders_effect)(p.senders.len() - known, known) } else { String::new() };
    section(ui, &mut take.senders, &(t.senders)(p.senders.len()), &effect, |ui| {
        scroll_list(ui, "senders", p.senders.iter().map(|a| format!("{} ← {}", a.name, a.aliases.join(", "))));
    });
    let (new, existing, missing) = note_counts(p, current, folders);
    let effect = (t.notes_effect)(new, existing, missing);
    let only_empty = &mut take.only_empty_notes;
    section(ui, &mut take.notes, &(t.notes)(p.folder_notes.len()), &effect, |ui| {
        if existing > 0 {
            theme::checkbox(ui, only_empty, t.only_empty_notes);
        }
        scroll_list(ui, "notes", p.folder_notes.iter().map(|n| format!("{} — {}", n.folder, n.note)));
    });
    section(ui, &mut take.rules, &(t.rules)(p.rules.len()), &replaces(current.rules.len()), |ui| {
        let lines = p.rules.iter().map(|r| {
            let mut line = format!("{}: {} → {}", r.name, r.keywords.join(", "), r.folder);
            if !r.require_keywords.is_empty() {
                line.push_str(&(t.rule_requires)(&r.require_keywords.join(", ")));
            }
            line
        });
        scroll_list(ui, "rules", lines);
    });
    let effect = if current.llm.instructions.trim().is_empty() { "" } else { t.instructions_effect };
    section(ui, &mut take.instructions, t.instructions, effect, |ui| {
        scroll_list(ui, "instructions", p.instructions.lines().map(String::from));
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.add(theme::primary_button(t.apply)).on_hover_text(t.apply_hint).clicked() {
            action = Some(PanelAction::Apply);
        }
        if ui.add(theme::secondary_button(t.discard)).clicked() {
            panel.proposal = None;
            panel.applied = None;
        }
    });
    action
}

fn hint_line(ui: &mut egui::Ui, s: &str) {
    ui.label(RichText::new(s).small().color(muted()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folders() -> FolderIndex {
        let all: Vec<String> =
            ["00_Eingang", "04_Finanzen", "04_Finanzen/Sparkasse", "05_Arbeit/Gehalt"].map(String::from).to_vec();
        FolderIndex { offered: all.clone(), all }
    }

    #[test]
    fn proposal_is_checked_before_use() {
        let p: Proposal = serde_json::from_value(json!({
            "filename_template": "{date}_{doctype}_{sender}",
            "date_format": "%y%m%d",
            "fallback_folder": "00_eingang",
            "doctypes": ["Rechnung", "Kontoauszug", "Bescheid", "rechnung"],
            "persons": [{ "name": "Max", "aliases": ["Maximilian", "Max"] }],
            "person_from_folder": true,
            "senders": [{ "name": "SWM", "aliases": ["Stadtwerke Musterstadt"] }],
            "folder_notes": [{ "folder": "04_Finanzen/Sparkasse", "note": "Girokonto" }, { "folder": "99_Gibts_nicht", "note": "x" }],
            "rules": [
                { "name": "Sparkasse", "keywords": ["1234567"], "folder": "04_Finanzen/Sparkasse/{year}" },
                { "name": "Auto", "keywords": ["Kennzeichen"], "folder": "03_Auto" },
                { "name": "", "keywords": [], "folder": "x" }
            ],
            "instructions": "- Versicherung vor Gegenstand"
        }))
        .unwrap();
        let mut cfg = Config { rules: vec![Rule::default()], ..Default::default() };
        let notes = apply(&p, &mut cfg, &folders(), Sections::default());
        assert_eq!(cfg.filename_template, "{date}_{doctype}_{sender}");
        assert_eq!(cfg.fallback_folder, "00_Eingang");
        assert_eq!(cfg.doctypes, ["Rechnung", "Kontoauszug", "Bescheid"]);
        assert_eq!(cfg.persons[0].aliases, ["Maximilian"]);
        assert!(cfg.person_from_folder);
        assert_eq!(cfg.folder_notes.get("04_Finanzen/Sparkasse").map(String::as_str), Some("Girokonto"));
        assert_eq!(cfg.rules.len(), 2);
        assert!(cfg.rules[0].enabled && !cfg.rules[1].enabled, "folder 03_Auto is missing");
        assert_eq!(cfg.llm.instructions, "- Versicherung vor Gegenstand");
        assert!(
            notes.iter().any(|n| n.contains("1 Ordnerbeschreibungen")) && notes.iter().any(|n| n.contains("03_Auto"))
        );
        assert!(notes.iter().any(|n| n.contains("1 bisherige Sonderregeln ersetzt")));

        let mut keep = Config::default();
        let before = keep.clone();
        apply(
            &p,
            &mut keep,
            &folders(),
            Sections {
                naming: false,
                doctypes: false,
                persons: false,
                senders: false,
                notes: false,
                rules: false,
                instructions: false,
                only_empty_notes: true,
            },
        );
        assert_eq!(keep, before, "nothing selected, nothing changed");

        // Existing descriptions stay as long as only empty ones are filled (`only_empty_notes`).
        let mut own = Config::default();
        own.folder_notes.insert("04_Finanzen/Sparkasse".into(), "Eigene Beschreibung".into());
        assert_eq!(note_counts(&p, &own, &folders()), (0, 1, 1));
        let notes = apply(&p, &mut own, &folders(), Sections::default());
        assert_eq!(own.folder_notes["04_Finanzen/Sparkasse"], "Eigene Beschreibung");
        assert!(notes.iter().any(|n| n.contains("1 vorhandene Ordnerbeschreibungen behalten")));
        apply(&p, &mut own, &folders(), Sections { only_empty_notes: false, ..Default::default() });
        assert_eq!(own.folder_notes["04_Finanzen/Sparkasse"], "Girokonto");
    }

    #[test]
    fn bad_values_are_reported() {
        let p =
            Proposal { filename_template: "{datum}_{doctype}".into(), date_format: "%Q".into(), ..Default::default() };
        let mut cfg = Config::default();
        let notes = apply(&p, &mut cfg, &folders(), Sections::default());
        assert_eq!(cfg.filename_template, Config::default().filename_template);
        assert!(notes.iter().any(|n| n.contains("{datum}")) && notes.iter().any(|n| n.contains("%Q")));
        assert!(valid_date_format("%y%m%d") && !valid_date_format(""));
    }

    #[test]
    fn schema_is_compact() {
        let s = schema();
        assert!(s["properties"]["folder_notes"]["items"]["properties"]["folder"].get("enum").is_none());
        assert_eq!(s["properties"]["rules"]["maxItems"], 20);
        assert_eq!(s["required"].as_array().unwrap().len(), 11);
        assert!(user_prompt("Text", &["A".into()]).contains("- A\n"));
    }
}
