//! Detail view of a document: preview (pages or text) and the form with the suggestion.

use std::path::Path;
use std::sync::Arc;

use eframe::egui::{self, Align, Color32, Layout, Margin, RichText, ScrollArea, Stroke, TextEdit, Vec2};

use crate::analysis::{self, Analysis, Origin};
use crate::archive::{self, FolderIndex};
use crate::config::Config;
use crate::i18n::{form as form_texts, general};
use crate::naming::{self, TemplateFields};
use crate::pages::{PageCache, Slot};
use crate::theme::{self, Arrow, text, tokens};
use tokens::{accent, muted, status_err, status_ok, status_warn};

#[derive(Default, Clone, PartialEq)]
pub struct Form {
    pub date: String,
    pub doctype: String,
    pub sender: String,
    pub subject: String,
    pub person: String,
    pub extra: String,
    pub folder: String,
    pub filename: String,
    /// The file name was edited by hand and is no longer generated automatically.
    pub filename_manual: bool,
    /// Parts left out of the file name (bits of [`Part`]); their values are kept.
    pub off: u8,
}

impl Form {
    pub fn from_analysis(a: &Analysis) -> Self {
        let s = &a.suggestion;
        Self {
            date: s.date.map(naming::DocDate::display).unwrap_or_default(),
            doctype: s.doctype.clone(),
            sender: s.sender.clone(),
            subject: s.subject.clone(),
            person: s.person.clone(),
            extra: s.extra.clone(),
            folder: s.folder.clone(),
            filename: String::new(),
            filename_manual: false,
            off: 0,
        }
    }

    /// Restores the file name parts as suggested; the name is built from them again.
    fn reset_name(&mut self, s: &analysis::Suggestion) {
        self.date = s.date.map(naming::DocDate::display).unwrap_or_default();
        self.doctype = s.doctype.clone();
        self.sender = s.sender.clone();
        self.subject = s.subject.clone();
        self.person = s.person.clone();
        self.extra = s.extra.clone();
        self.filename_manual = false;
        self.off = 0;
    }

    /// Is the part in the file name (or is it switched off)?
    fn is_on(&self, part: Part) -> bool {
        self.off & part.bit() == 0
    }

    fn value(&self, part: Part) -> &String {
        match part {
            Part::Date => &self.date,
            Part::Doctype => &self.doctype,
            Part::Sender => &self.sender,
            Part::Subject => &self.subject,
            Part::Person => &self.person,
            Part::Extra => &self.extra,
        }
    }

    fn value_mut(&mut self, part: Part) -> &mut String {
        match part {
            Part::Date => &mut self.date,
            Part::Doctype => &mut self.doctype,
            Part::Sender => &mut self.sender,
            Part::Subject => &mut self.subject,
            Part::Person => &mut self.person,
            Part::Extra => &mut self.extra,
        }
    }

    /// Parts with a fixed name (`date`, `doctype` …) switched off in the file name, for the log.
    pub fn off_fields(&self) -> Vec<String> {
        Part::ALL.into_iter().filter(|&p| !self.is_on(p)).map(|p| p.key().to_string()).collect()
    }

    fn fields(&self) -> TemplateFields<'_> {
        let on = |part: Part| if self.is_on(part) { self.value(part).as_str() } else { "" };
        TemplateFields {
            date: if self.is_on(Part::Date) { analysis::parse_date(&self.date) } else { None },
            doctype: on(Part::Doctype),
            sender: on(Part::Sender),
            subject: on(Part::Subject),
            person: on(Part::Person),
            extra: on(Part::Extra),
        }
    }
}

/// File name from the fields; program files keep their name.
pub fn auto_filename(cfg: &Config, form: &Form, keep_name: bool, original_stem: &str) -> String {
    if keep_name {
        return original_stem.to_string();
    }
    analysis::suggested_filename(cfg, &form.fields(), &form.folder)
}

/// Properties for Windows Search from the fields and the summary.
pub fn search_tags(form: &Form, summary: &str) -> crate::tags::Tags {
    crate::tags::Tags::build(&crate::tags::Source {
        doctype: &form.doctype,
        sender: &form.sender,
        subject: &form.subject,
        person: &form.person,
        extra: &form.extra,
        year: naming::reference_year(&form.subject, naming::DocDate::parse(&form.date)),
        folder: &form.folder,
        summary,
    })
}

pub enum FormAction {
    Confirm,
    Skip,
}

/// What the form needs besides the document.
pub struct FormCtx<'a> {
    pub cfg: &'a Config,
    pub folders: &'a FolderIndex,
    pub filter: &'a mut String,
    pub path: &'a Path,
    pub keep_name: bool,
    pub doc_id: u64,
    /// Error from the last filing.
    pub error: Option<&'a str>,
}

/// Why the document cannot be filed right now; `None` when everything is ready.
pub fn file_blocker(cfg: &Config, form: &Form) -> Option<String> {
    let t = form_texts::t();
    let root = cfg.archive_root();
    if !root.is_dir() {
        return Some((t.blocker_archive)(&root.display().to_string()));
    }
    if let Err(e) = archive::safe_relative_folder(&form.folder) {
        return Some((t.blocker_folder)(&e.to_string()));
    }
    if form.filename.trim().is_empty() {
        return Some(t.blocker_name.into());
    }
    if !form.filename_manual
        && form.is_on(Part::Date)
        && !form.date.trim().is_empty()
        && analysis::parse_date(&form.date).is_none()
    {
        return Some(t.date_unrecognized.into());
    }
    None
}

enum FolderState {
    Exists,
    NewYear,
    New,
    Invalid(String),
}

fn folder_state(folder: &str, folders: &FolderIndex, root: &Path) -> FolderState {
    match archive::safe_relative_folder(folder) {
        Err(e) => FolderState::Invalid(format!("{e}")),
        Ok(rel) if folder.trim().is_empty() || folders.exists(folder) || root.join(&rel).is_dir() => {
            FolderState::Exists
        }
        Ok(_) if archive::is_year_folder(folder) => FolderState::NewYear,
        Ok(_) => FolderState::New,
    }
}

/// Suggestion for a field: where it came from and what its value was.
#[derive(Clone, Copy)]
struct Proposed<'a> {
    origin: Origin,
    value: &'a str,
}

fn pr(origin: Origin, value: &str) -> Proposed<'_> {
    Proposed { origin, value }
}

/// Height of a table row.
const ROW_H: f32 = theme::CONTROL;
/// Inner padding of the value cells: text without its own frame, the table draws the lines.
const CELL_MARGIN: Margin = Margin::symmetric(4, 7);
/// Inner padding of the label cells.
const LABEL_PAD: Margin = Margin { left: 8, right: 2, top: 3, bottom: 0 };
/// Arrow for the drop-down list at the end of a value cell.
const LIST_BUTTON: f32 = 24.0;
/// Label column at least wide enough for the *changed* marker to fit below the name.
const LABEL_MIN: f32 = 64.0;

/// Width of a text as it is drawn.
pub fn text_width(ui: &egui::Ui, text: RichText) -> f32 {
    egui::WidgetText::from(text)
        .into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body)
        .size()
        .x
}

fn changed_text() -> RichText {
    RichText::new(theme::with_icon(theme::icon::UNDO, form_texts::t().changed)).size(text::OVERLINE).color(accent())
}

fn off_text() -> RichText {
    RichText::new(form_texts::t().off_in_name).size(text::OVERLINE).color(muted())
}

/// Origin of the suggestion (`Suggestion::source`: "AI (model)", "heuristics", "manual") in the UI
/// language; anything unknown stays as it is.
fn source_text(source: &str) -> String {
    let t = form_texts::t();
    match source {
        "heuristics" => t.source_value_heuristics.to_string(),
        "manual" => t.source_value_manual.to_string(),
        _ => match source.strip_prefix("AI (").and_then(|rest| rest.strip_suffix(')')) {
            Some(model) => (t.source_value_ai)(model),
            None => source.to_string(),
        },
    }
}

/// Shortcut with the key names of the current language, e.g. "Ctrl+Enter".
fn keys(names: &[&str]) -> String {
    names.join("+")
}

/// Label of a table row: the field name on top, the origin of the suggestion below ("AI", "Rule" …).
/// If the value was changed, it says *changed* there instead; clicking it restores the suggestion.
/// Returns `true` on that click.
fn label_cell(ui: &mut egui::Ui, label: &str, value: &str, proposed: Proposed, in_name: bool) -> bool {
    let t = form_texts::t();
    ui.spacing_mut().item_spacing.y = 1.0;
    ui.label(theme::field_label(label));
    if value.trim() != proposed.value.trim() {
        let hover = if proposed.value.is_empty() { t.restore_empty.to_string() } else { (t.restore)(proposed.value) };
        let link = egui::Label::new(changed_text()).truncate().sense(egui::Sense::CLICK);
        ui.add(link).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(hover).clicked()
    } else {
        if !in_name {
            ui.add(egui::Label::new(off_text()).truncate()).on_hover_text(t.off_hover);
        } else if proposed.origin != Origin::None {
            let origin = RichText::new(proposed.origin.label()).size(text::OVERLINE).color(muted());
            ui.add(egui::Label::new(origin).truncate()).on_hover_text((t.origin_hover)(proposed.origin.label()));
        }
        false
    }
}

/// Value cell: frameless text, editable in place; for doctype, person and extra with an arrow for the
/// drop-down list, free text stays possible. A filled background on hover, an accent line at the bottom
/// while editing.
fn value_cell(
    ui: &mut egui::Ui,
    width: f32,
    id: (u8, u64),
    value: &mut String,
    hint: &str,
    choices: &[String],
) -> bool {
    let t = form_texts::t();
    let list = if choices.is_empty() { 0.0 } else { LIST_BUTTON };
    // The field width counts without the inner padding.
    let w = (width - list - CELL_MARGIN.sum().x).max(30.0);
    let mut changed = false;
    let behind = ui.painter().add(egui::Shape::Noop);
    let cell = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let edit = ui.add(
            TextEdit::singleline(value)
                .hint_text(hint)
                .frame(egui::Frame::NONE.inner_margin(CELL_MARGIN))
                .desired_width(w),
        );
        changed |= edit.changed();
        if !choices.is_empty() {
            let arrow = egui::Button::new(theme::icon_text(theme::icon::CHEVRON_DOWN, theme::ICON_INLINE, muted()))
                .frame(false)
                .min_size(Vec2::new(LIST_BUTTON, ROW_H));
            egui::containers::menu::MenuButton::from_button(arrow)
                .ui(ui, |ui| {
                    theme::list_style(ui);
                    ui.set_min_width(220.0);
                    ScrollArea::vertical().id_salt(("choices", id)).max_height(380.0).show(ui, |ui| {
                        for c in choices {
                            let shown = if c.is_empty() { t.empty_choice } else { c.as_str() };
                            if ui.selectable_label(value == c, shown).clicked() {
                                *value = c.clone();
                                changed = true;
                                ui.close();
                            }
                        }
                    });
                })
                .0
                .on_hover_text(t.pick_from_list);
        }
        edit.has_focus()
    });
    let rect = cell.response.rect;
    if cell.inner {
        ui.painter().set(behind, egui::Shape::rect_filled(rect, 0.0, tokens::accent_subtle()));
        ui.painter().hline(rect.x_range(), rect.bottom() - 1.0, Stroke::new(2.0_f32, accent()));
    } else if ui.rect_contains_pointer(rect) {
        ui.painter().set(behind, egui::Shape::rect_filled(rect, 0.0, tokens::hover()));
    }
    changed
}

/// One row of the table with the values of the suggestion.
struct FieldSpec<'a> {
    part: Part,
    hint: &'static str,
    choices: &'a [String],
    proposed: Proposed<'a>,
}

/// Values of the suggestion as a table like in a spreadsheet: label on the left, value on the right, thin
/// lines instead of a frame around each field; two pairs per row from medium width on. Returns whether a
/// value changed.
fn fields_table(ui: &mut egui::Ui, form: &mut Form, specs: &[FieldSpec], doc_id: u64) -> bool {
    let line = Stroke::new(1.0_f32, tokens::line());
    // Label width per column: as wide as the longest label; the line below it gets truncated.
    let label_w = |specs: &mut dyn Iterator<Item = &FieldSpec>| {
        let widest = specs.map(|s| text_width(ui, theme::field_label(s.part.label()))).fold(LABEL_MIN, f32::max);
        widest.ceil() + LABEL_PAD.sum().x
    };
    let (l1, l2) = (label_w(&mut specs.iter().step_by(2)), label_w(&mut specs.iter().skip(1).step_by(2)));
    let mut changed = false;
    let table = egui::Frame::new().stroke(Stroke::new(1.0_f32, tokens::line())).fill(tokens::surface());
    table.corner_radius(theme::radius::LG).show(ui, |ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let width = ui.available_width();
        let two_value_w = ((width - l1 - l2 - 1.0) / 2.0).floor();
        let cols = if two_value_w >= 120.0 { 2 } else { 1 };
        let rows: Vec<&[FieldSpec]> = specs.chunks(cols).collect();
        for (r, row) in rows.iter().enumerate() {
            let resp = ui.horizontal(|ui| {
                for (i, spec) in row.iter().enumerate() {
                    let (label_w, value_w) = match cols {
                        2 => (if i == 0 { l1 } else { l2 }, two_value_w),
                        _ => (l1.max(l2), (width - l1.max(l2)).floor()),
                    };
                    if i > 0 {
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, ROW_H), egui::Sense::hover());
                        ui.painter().vline(rect.center().x, rect.y_range(), line);
                    }
                    let in_name = form.is_on(spec.part);
                    let cell = Vec2::new(label_w, ROW_H);
                    let reset = ui
                        .allocate_ui_with_layout(cell, Layout::top_down(Align::Min), |ui| {
                            ui.set_min_size(cell);
                            egui::Frame::new()
                                .inner_margin(LABEL_PAD)
                                .show(ui, |ui| {
                                    ui.set_width(label_w - LABEL_PAD.sum().x);
                                    let value = form.value(spec.part);
                                    label_cell(ui, spec.part.label(), value, spec.proposed, in_name)
                                })
                                .inner
                        })
                        .inner;
                    let value = form.value_mut(spec.part);
                    if reset {
                        *value = spec.proposed.value.to_string();
                        changed = true;
                    }
                    // The id belongs to the part, not to its label: the label changes with the language.
                    let id = (spec.part as u8, doc_id);
                    changed |= value_cell(ui, value_w, id, value, spec.hint, spec.choices);
                }
            });
            if r + 1 < rows.len() {
                ui.painter().hline(ui.max_rect().x_range(), resp.response.rect.bottom(), line);
            }
        }
    });
    changed
}

fn in_list(value: &str, list: &[String]) -> bool {
    value.trim().is_empty() || list.iter().any(|d| archive::normalize(d) == archive::normalize(value.trim()))
}

/// Form with the suggestion. The fields first (with their own scroll bar), below them, always visible, the
/// filing target and the buttons – in this order so that Tab leads from the first field to the *File* button.
pub fn form_ui(ui: &mut egui::Ui, form: &mut Form, analysis: &Analysis, ctx: FormCtx) -> Option<FormAction> {
    let FormCtx { cfg, folders, filter, path, keep_name, doc_id, error } = ctx;
    let t = form_texts::t();
    let s = &analysis.suggestion;
    let ext = crate::ingest::extension(path).unwrap_or_default();
    let original_stem = analysis::original_stem(path);
    let root = cfg.archive_root();
    let mut action = None;
    // Footer height from the last frame: this way the fields get the rest.
    let footer_id = egui::Id::unique(("form-footer", doc_id));
    let footer_h: f32 = ui.data(|d| d.get_temp(footer_id)).unwrap_or(300.0);
    let fields_h = (ui.available_height() - footer_h - 12.0).max(120.0);

    ScrollArea::vertical().id_salt(("form", doc_id)).auto_shrink([false, false]).max_height(fields_h).show(ui, |ui| {
        ui.set_width(ui.available_width() - 8.0);
        if !root.is_dir() {
            egui::Frame::new()
                .fill(tokens::status_err_bg())
                .stroke(Stroke::new(1.0_f32, status_err()))
                .corner_radius(theme::radius::MD)
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(t.archive_unreachable).family(theme::semibold()).color(status_err()));
                    ui.label((t.archive_missing)(&root.display().to_string()));
                });
            ui.add_space(8.0);
        }
        // Origin of the suggestion
        ui.horizontal_wrapped(|ui| {
            // `source` is analysis data ("AI …", "manual"): comparisons use it, the UI shows its
            // translation.
            let label = if s.source.starts_with("AI (") {
                t.source_ai
            } else if s.source == "manual" {
                t.source_manual
            } else {
                t.source_no_ai
            };
            let hover =
                if s.source.starts_with("AI (") { source_text(&s.source) } else { t.source_no_ai_hover.to_string() };
            theme::chip(ui, label, muted()).on_hover_text(hover);
            if s.rule.is_some() {
                theme::chip(ui, t.rule, muted()).on_hover_text(s.rule.as_deref().unwrap_or_default());
            }
            // A percentage only if the AI delivered one; otherwise it would be made up.
            if s.confidence_from_ai {
                let pct = s.confidence * 100.0;
                let (color, sure) = if s.confidence >= 0.75 {
                    (muted(), (t.confidence)(pct))
                } else if s.confidence >= 0.45 {
                    (status_warn(), (t.confidence_check)(pct))
                } else {
                    (status_err(), (t.confidence_unsure)(pct))
                };
                ui.label(RichText::new(sure).small().color(color));
            }
            let mut meta = vec![analysis.kind.label().to_string()];
            if let Some(engine) = analysis.ocr_engine.as_ref().filter(|e| *e != analysis.kind.label()) {
                meta.push(engine.clone());
            }
            meta.push((t.seconds)(analysis.duration.as_secs_f32()));
            let meta = ui.label(RichText::new(meta.join(" · ")).small().color(muted()));
            if let Some(stats) = &analysis.ai_stats {
                meta.on_hover_text(stats);
            }
        });
        if let Some(rule) = &s.rule {
            ui.label(RichText::new(rule).small().color(muted()));
        }
        if !s.summary.is_empty() {
            // Summary at most two lines; all of it when expanded. Otherwise the fields would hardly get any room.
            ui.add_space(4.0);
            let open_id = egui::Id::unique(("summary-open", doc_id));
            let mut open: bool = ui.data(|d| d.get_temp(open_id)).unwrap_or(false);
            egui::Frame::new()
                .fill(tokens::surface_muted())
                .corner_radius(theme::radius::MD)
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let format = egui::TextFormat {
                        font_id: egui::FontId::proportional(text::BODY),
                        color: ui.visuals().text_color(),
                        ..Default::default()
                    };
                    let mut job = egui::text::LayoutJob::single_section(s.summary.clone(), format);
                    job.wrap = egui::text::TextWrapping {
                        max_width: ui.available_width(),
                        max_rows: if open { usize::MAX } else { 2 },
                        break_anywhere: false,
                        overflow_character: Some('…'),
                    };
                    let galley = ui.fonts_mut(|f| f.layout_job(job));
                    let long = open || galley.elided;
                    // Clicking the text expands and collapses it; a separate line for that would cost space.
                    let sense = if long { egui::Sense::CLICK } else { egui::Sense::hover() };
                    let resp = ui.add(egui::Label::new(galley).sense(sense));
                    if long {
                        let hover = if open { t.summary_less } else { t.summary_more };
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(hover).clicked() {
                            open = !open;
                        }
                    }
                });
            ui.data_mut(|d| d.insert_temp(open_id, open));
        }
        let notes: Vec<String> = analysis.corrections.iter().map(|c| (t.correction)(c)).collect();
        if !analysis.warnings.is_empty() || !notes.is_empty() {
            ui.add_space(4.0);
            for w in &analysis.warnings {
                ui.label(RichText::new(w).small().color(status_warn()));
            }
            for n in &notes {
                ui.label(RichText::new(n).small().color(muted()));
            }
        }
        ui.add_space(8.0);

        // Values of the suggestion as a table, in schema order.
        let o = s.origin;
        let proposed_date = s.date.map(naming::DocDate::display).unwrap_or_default();
        let mut persons: Vec<String> = vec![String::new()];
        persons.extend(cfg.persons.iter().map(|p| p.name.clone()));
        // "Familie" is a value for the file name as in `naming`, not a label: not translated.
        if cfg.persons.len() > 1 {
            persons.push("Familie".into());
        }
        let mut extras: Vec<String> = vec![String::new()];
        extras.extend(cfg.extras.iter().cloned());
        let spec = |part, hint, choices, proposed| FieldSpec { part, hint, choices, proposed };
        let specs = [
            spec(Part::Date, t.hint_date, &[][..], pr(o.date, &proposed_date)),
            spec(Part::Doctype, t.hint_doctype, &cfg.doctypes[..], pr(o.doctype, &s.doctype)),
            spec(Part::Sender, t.hint_sender, &[][..], pr(o.sender, &s.sender)),
            spec(Part::Subject, t.hint_subject, &[][..], pr(o.subject, &s.subject)),
            spec(Part::Person, t.hint_optional, &persons[..], pr(o.person, &s.person)),
            spec(Part::Extra, t.hint_extra, &extras[..], pr(o.extra, &s.extra)),
        ];
        let changed = fields_table(ui, form, &specs, doc_id);
        if !form.date.trim().is_empty() && analysis::parse_date(&form.date).is_none() {
            ui.label(RichText::new(t.date_unrecognized_examples).small().color(status_err()));
        } else if form.date.trim().is_empty() {
            ui.label(RichText::new(t.no_date).small().color(muted()));
        }
        if !in_list(&form.doctype, &cfg.doctypes) {
            ui.label(RichText::new(t.new_doctype).small().color(muted()));
        }

        if changed && !form.filename_manual {
            form.filename = auto_filename(cfg, form, keep_name, &original_stem);
        }

        if analysis.page_count > analysis.previews.len() && !analysis.previews.is_empty() {
            let analysed = (t.pages_analysed)(analysis.previews.len(), analysis.page_count);
            ui.label(RichText::new(analysed).small().color(muted()));
        }
    });
    ui.add_space(4.0);
    let footer = ui.scope(|ui| {
        let ctx = FooterCtx {
            cfg,
            folders,
            filter,
            path,
            keep_name,
            doc_id,
            error,
            ext: &ext,
            original_stem: &original_stem,
        };
        action = footer_ui(ui, form, s, ctx);
    });
    let h = footer.response.rect.height();
    if (h - footer_h).abs() > 1.0 {
        ui.data_mut(|d| d.insert_temp(footer_id, h));
        ui.ctx().request_repaint();
    }
    action
}

struct FooterCtx<'a> {
    cfg: &'a Config,
    folders: &'a FolderIndex,
    filter: &'a mut String,
    path: &'a Path,
    keep_name: bool,
    doc_id: u64,
    error: Option<&'a str>,
    ext: &'a str,
    original_stem: &'a str,
}

/// Filing target (folder and file name) and the buttons.
fn footer_ui(ui: &mut egui::Ui, form: &mut Form, s: &analysis::Suggestion, ctx: FooterCtx) -> Option<FormAction> {
    let FooterCtx { cfg, folders, filter, path, keep_name, doc_id, error, ext, original_stem } = ctx;
    let t = form_texts::t();
    let g = general::t();
    let root = cfg.archive_root();
    let mut action = None;
    // Reasons for the folder choice: one bullet per stage, plus the folder's description. *Why?* shows them
    // on hover; a click expands them below the folder (remembered per document until Scrinium closes).
    let mut why: Vec<String> = s.folder_why.iter().map(|r| format!("• {r}")).collect();
    why.extend(analysis::folder_note(&cfg.folder_notes, &form.folder).map(|n| (t.why_note)(n)));
    let why_id = egui::Id::unique(("why-open", doc_id));
    let mut why_open = ui.data(|d| d.get_temp::<bool>(why_id)).unwrap_or(false);
    // Filing target as its own card: folder and file name are the result you check before filing.
    egui::Frame::new()
        .fill(tokens::highlight())
        .stroke(Stroke::new(1.0_f32, accent()))
        .corner_radius(theme::radius::LG)
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let mut folder_changed = false;
            ui.horizontal(|ui| {
                theme::folder_icon(ui, muted());
                ui.label(theme::field_label(t.destination));
                ui.spacing_mut().item_spacing.x = 4.0;
                if form.folder != s.folder {
                    ui.label(RichText::new(format!("· {}", t.changed)).size(text::SMALL).color(muted()));
                } else {
                    if s.origin.folder != analysis::Origin::None {
                        ui.label(
                            RichText::new(format!("· {}", s.origin.folder.label())).size(text::SMALL).color(muted()),
                        );
                    }
                    if !why.is_empty() {
                        let chevron = if why_open { theme::icon::CHEVRON_UP } else { theme::icon::CHEVRON_DOWN };
                        let link = RichText::new(format!("{} {chevron}", t.why)).size(text::SMALL).color(accent());
                        // Click area without keyboard focus (R7).
                        let resp = ui
                            .add(egui::Label::new(link).selectable(false).sense(egui::Sense::CLICK))
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        let hover = match why_open {
                            true => t.why_hide.to_string(),
                            false => format!("{}\n\n{}", why.join("\n"), t.why_show),
                        };
                        if resp.on_hover_text(hover).clicked() {
                            why_open = !why_open;
                            ui.data_mut(|d| d.insert_temp(why_id, why_open));
                        }
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (label, color) = match folder_state(&form.folder, folders, &root) {
                        _ if !root.is_dir() => (t.archive_unreachable, status_err()),
                        FolderState::Exists => (t.folder_exists, status_ok()),
                        FolderState::NewYear => (t.folder_new_year, status_warn()),
                        FolderState::New => (t.folder_new, status_warn()),
                        FolderState::Invalid(_) => (t.folder_invalid, status_err()),
                    };
                    let states =
                        [t.archive_unreachable, t.folder_exists, t.folder_new_year, t.folder_new, t.folder_invalid];
                    theme::chip_min(ui, label, color, theme::chip_width(ui, &states, true));
                    let changed = form.folder != s.folder;
                    let shown =
                        if s.folder.is_empty() { t.archive.to_string() } else { s.folder.replace('/', " › ") };
                    let hover = match changed {
                        true => (t.back_to_folder)(&shown),
                        false => t.is_proposed_folder.to_string(),
                    };
                    if theme::icon_button(ui, theme::icon::UNDO, changed, &hover) {
                        form.folder = s.folder.clone();
                        folder_changed = true;
                    }
                });
            });
            folder_changed |= folder_search_field(ui, &mut form.folder, folders, cfg, filter, doc_id);
            folder_changed |= folder_crumbs(ui, form, folders, doc_id);
            // Expanded: all the reasons. Otherwise, for an unsure suggestion or the fallback folder, the first
            // three stages, so you don't have to hover to find them.
            let unsure = (s.confidence_from_ai && s.confidence < 0.6) || s.origin.folder == analysis::Origin::Fallback;
            if form.folder == s.folder && (why_open || unsure) {
                let shown = if why_open { why.len() } else { s.folder_why.len().min(3) };
                for line in why.iter().take(shown) {
                    ui.add(egui::Label::new(RichText::new(line).small().color(muted())).wrap());
                }
            }
            if let FolderState::Invalid(e) = folder_state(&form.folder, folders, &root) {
                ui.label(RichText::new(e).small().color(status_err()));
            }
            if matches!(folder_state(&form.folder, folders, &root), FolderState::New) {
                let near = similar_folders(&form.folder, &folders.all);
                if !near.is_empty() {
                    let mut picked = None;
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(t.did_you_mean).small().color(status_warn()));
                        for f in &near {
                            if theme::ghost_button(ui, f.as_str()).on_hover_text(t.take_folder).clicked() {
                                picked = Some(f.clone());
                            }
                        }
                    });
                    if let Some(f) = picked {
                        form.folder = f;
                        folder_changed = true;
                    }
                }
            }
            if folder_changed && !form.filename_manual {
                form.filename = auto_filename(cfg, form, keep_name, original_stem);
            }
            folder_files(ui, &root, form, ext, doc_id);
            ui.add_space(8.0);
            // Suggested name in the current folder (folder templates belong to the folder).
            let mut proposed = form.clone();
            proposed.reset_name(s);
            let proposed_name = auto_filename(cfg, &proposed, keep_name, original_stem);
            ui.horizontal(|ui| {
                theme::file_icon(ui, muted());
                ui.label(theme::field_label(t.file_name));
                let len = form.filename.chars().count();
                let over = cfg.compact_filenames && len > cfg.max_filename_length;
                ui.label(
                    RichText::new(format!("{len}/{}", cfg.max_filename_length)).size(text::SMALL).color(if over {
                        status_err()
                    } else {
                        muted()
                    }),
                );
                if keep_name {
                    ui.label(RichText::new(t.program_file).small().color(muted()));
                    return;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let changed = form.filename != proposed_name;
                    let hover = match changed {
                        true => (t.back_to_name)(&proposed_name),
                        false => t.is_proposed_name.to_string(),
                    };
                    if theme::icon_button(ui, theme::icon::UNDO, changed, &hover) {
                        form.reset_name(s);
                        form.filename = proposed_name.clone();
                    }
                    let other = form.filename != original_stem;
                    let original = ui.add_enabled_ui(other, |ui| theme::ghost_button(ui, t.original_name)).inner;
                    let original = original
                        .on_hover_text((t.take_original)(original_stem))
                        .on_disabled_hover_text(t.is_original_name);
                    if original.clicked() {
                        form.filename = original_stem.to_string();
                        form.filename_manual = true;
                    }
                });
            });
            let target = archive::safe_relative_folder(&form.folder).ok().map(|rel| {
                let stem = archive::sanitize_component(&form.filename);
                // "Dokument" is the file name `archive` files under – not a label.
                let stem = if stem.is_empty() { "Dokument".to_string() } else { stem };
                let target = archive::unique_target(&root.join(&rel), &stem, ext);
                let renamed = target.file_stem().is_some_and(|t| t.to_string_lossy() != stem);
                (target, renamed)
            });
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 64.0 - 2.0 * theme::FIELD_MARGIN).max(80.0);
                let edit = TextEdit::singleline(&mut form.filename)
                    .font(egui::FontId::monospace(text::LARGE))
                    .desired_width(w);
                let resp = theme::field(ui, edit);
                if let Some((target, _)) = &target {
                    resp.clone().on_hover_text(target.display().to_string());
                }
                if resp.changed() {
                    form.filename_manual = true;
                }
                ui.label(RichText::new(format!(".{ext}")).font(egui::FontId::monospace(text::LARGE)).color(muted()));
            });
            // Name parts to click. They stay (faded) even with a custom name, so nothing moves; changing a part
            // rebuilds the name from the parts.
            let own_name = form.filename_manual;
            if !keep_name && name_parts(ui, form, cfg, doc_id, own_name) {
                form.filename_manual = false;
                form.filename = auto_filename(cfg, form, keep_name, original_stem);
            }
            if let Some((target, true)) = &target {
                let v = target.file_stem().map(|t| t.to_string_lossy().into_owned()).unwrap_or_default();
                ui.label(RichText::new((t.name_taken)(&format!("{v}.{ext}"))).small().color(status_warn()));
            }
            // No hint for the original name: it was kept on purpose.
            if cfg.compact_filenames
                && !keep_name
                && form.filename != original_stem
                && form.filename.contains(|c: char| c.is_whitespace() || "&+#(),;'!?".contains(c))
            {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t.special_chars).small().color(status_warn()));
                    if theme::ghost_button(ui, &theme::with_icon(theme::icon::ERASER, t.clean_up)).clicked() {
                        form.filename =
                            form.filename.split('_').map(naming::compact_component).collect::<Vec<_>>().join("_");
                        form.filename = naming::cleanup_separators(&form.filename);
                    }
                });
            }
            if cfg.write_tags && crate::tags::supports(path) {
                let tags = search_tags(form, &s.summary);
                let hover = (t.tags_hover)(&tags.title, &tags.subject, &tags.keywords.join("; "));
                ui.horizontal_wrapped(|ui| {
                    ui.label(theme::field_label(t.tags)).on_hover_text(hover.as_str());
                    let list = RichText::new(tags.keywords.join(" · ")).size(text::SMALL).color(muted());
                    ui.add(egui::Label::new(list).wrap()).on_hover_text(hover.as_str());
                });
            }
        });
    if let Some(e) = error {
        ui.add_space(8.0);
        egui::Frame::new()
            .fill(tokens::status_err_bg())
            .stroke(Stroke::new(1.0_f32, status_err()))
            .corner_radius(theme::radius::MD)
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(theme::with_icon(theme::icon::ERROR, t.filing_failed))
                        .family(theme::semibold())
                        .color(status_err()),
                );
                ui.label(e);
            });
    }
    ui.add_space(8.0);
    let blocker = file_blocker(cfg, form);
    let file_keys = keys(&[g.key_ctrl, g.key_enter]);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let new_folder = matches!(folder_state(&form.folder, folders, &root), FolderState::New);
        let verb = match (cfg.copy_instead_of_move, new_folder) {
            (true, false) => t.file_copy,
            (true, true) => t.file_copy_new_folder,
            (false, false) => t.file,
            (false, true) => t.file_new_folder,
        };
        let resp = ui.add_enabled(blocker.is_none(), theme::primary_button(verb).min_size(Vec2::new(160.0, 0.0)));
        let resp =
            resp.on_hover_text(file_keys.as_str()).on_disabled_hover_text(blocker.as_deref().unwrap_or_default());
        if resp.clicked() {
            action = Some(FormAction::Confirm);
        }
        if ui
            .add(theme::secondary_button(t.later))
            .on_hover_text((t.later_hover)(&keys(&[g.key_ctrl, g.key_shift, g.key_enter])))
            .clicked()
        {
            action = Some(FormAction::Skip);
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| match &blocker {
            Some(why) => {
                ui.add(egui::Label::new(RichText::new(why).small().color(status_err())).truncate());
            }
            None => {
                ui.label(RichText::new(&file_keys).small().color(muted()));
            }
        });
    });
    action
}

/// Files of a folder for the preview, cached: the folder is read at most every two seconds, names only
/// (no metadata, cloud sync placeholders stay untouched).
/// `None` if the folder does not exist (yet) or cannot be read.
fn folder_listing(ui: &egui::Ui, dir: &Path) -> Option<Arc<Vec<String>>> {
    let id = egui::Id::unique(("folder-files", dir));
    let now = ui.input(|i| i.time);
    if let Some((at, list)) = ui.data(|d| d.get_temp::<(f64, Option<Arc<Vec<String>>>)>(id))
        && now - at < 2.0
    {
        return list;
    }
    let list = std::fs::read_dir(dir).ok().map(|entries| {
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| {
                !n.starts_with('.')
                    && !n.starts_with("~$")
                    && !["desktop.ini", "thumbs.db"].contains(&n.to_lowercase().as_str())
            })
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        Arc::new(names)
    });
    ui.data_mut(|d| d.insert_temp(id, (now, list.clone())));
    list
}

/// Preview of the files in the target folder, with the new name highlighted in its place. Expanded or
/// collapsed applies to all documents.
fn folder_files(ui: &mut egui::Ui, root: &Path, form: &Form, ext: &str, doc_id: u64) {
    let Ok(rel) = archive::safe_relative_folder(&form.folder) else { return };
    if !root.is_dir() {
        return;
    }
    let t = form_texts::t();
    let dir = root.join(rel);
    let Some(files) = folder_listing(ui, &dir) else {
        ui.label(RichText::new(t.new_folder_no_files).small().color(muted()));
        return;
    };
    if files.is_empty() {
        ui.label(RichText::new(t.no_files_yet).small().color(muted()));
        return;
    }
    // Open by itself only in tall windows; otherwise the list would take the room of the suggestion's values.
    let open_id = egui::Id::unique("folder-files-open");
    let tall = ui.ctx().content_rect().height() >= 1000.0;
    let mut open: bool = ui.data(|d| d.get_temp(open_id)).unwrap_or(tall);
    let count = (t.files_in_folder)(files.len());
    let chevron = if open { theme::icon::CHEVRON_DOWN } else { theme::icon::CHEVRON_RIGHT };
    let header = egui::Label::new(RichText::new(theme::with_icon(chevron, &count)).small().color(muted()))
        .sense(egui::Sense::CLICK);
    let hover = if open { t.collapse_list } else { t.show_files };
    if ui.add(header).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(hover).clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(open_id, open));
    }
    if !open {
        return;
    }
    let stem = archive::sanitize_component(&form.filename);
    let stem = if stem.is_empty() { "Dokument".to_string() } else { stem };
    let new = archive::unique_target(&dir, &stem, ext).file_name().map(|n| n.to_string_lossy().into_owned());
    let mut rows: Vec<(&str, bool)> = files.iter().map(|n| (n.as_str(), false)).collect();
    if let Some(new) = &new {
        let at = rows.partition_point(|(n, _)| n.to_lowercase() < new.to_lowercase());
        rows.insert(at, (new.as_str(), true));
    }
    // Scroll to the new name when it has changed – otherwise the list could not be scrolled by hand.
    let seen_id = egui::Id::unique(("folder-files-seen", doc_id));
    let key = format!("{}/{}", dir.display(), new.as_deref().unwrap_or_default());
    let scroll = ui.data(|d| d.get_temp::<String>(seen_id)).as_deref() != Some(key.as_str());
    egui::Frame::new()
        .fill(tokens::surface())
        .stroke(Stroke::new(1.0_f32, tokens::line()))
        .corner_radius(theme::radius::LG)
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ScrollArea::vertical().id_salt(("folder-files", doc_id)).max_height(100.0).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for (name, is_new) in rows {
                    if is_new {
                        let text = RichText::new((t.new_file)(name)).small().family(theme::semibold()).color(accent());
                        let resp = ui.add(egui::Label::new(text).truncate());
                        if scroll {
                            resp.scroll_to_me(Some(Align::Center));
                        }
                    } else {
                        ui.add(egui::Label::new(RichText::new(name).small().color(tokens::fg())).truncate());
                    }
                }
            });
        });
    if scroll {
        ui.data_mut(|d| d.insert_temp(seen_id, key));
    }
}

/// Names of the direct subfolders of `parent` (empty = top level), in list order.
fn children(folders: &[String], parent: &str) -> Vec<String> {
    let parent_parts = archive::split_folder(parent);
    let key = archive::normalize(&parent_parts.join("/"));
    let mut out: Vec<String> = Vec::new();
    for f in folders {
        let parts = archive::split_folder(f);
        if parts.len() <= parent_parts.len() || archive::normalize(&parts[..parent_parts.len()].join("/")) != key {
            continue;
        }
        let name = parts[parent_parts.len()].to_string();
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// Folder path as clickable breadcrumbs: a folder part shows its siblings and *File here*, the trailing
/// "›" the subfolders. Returns whether the folder changed.
fn folder_crumbs(ui: &mut egui::Ui, form: &mut Form, folders: &FolderIndex, doc_id: u64) -> bool {
    let t = form_texts::t();
    let parts: Vec<String> = archive::split_folder(&form.folder).iter().map(|s| s.to_string()).collect();
    let mut chosen: Option<String> = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        // Level 0 is the archive itself; each further level is a folder part – as a chip like the name parts.
        for level in 0..=parts.len() {
            let parent = parts[..level.saturating_sub(1)].join("/");
            let last = level == parts.len();
            let label = if level == 0 { t.archive.to_string() } else { parts[level - 1].clone() };
            let text = if last && level > 0 {
                RichText::new(&label).size(text::LARGE).family(theme::semibold()).color(tokens::fg())
            } else if level == 0 {
                RichText::new(theme::with_icon(theme::icon::FOLDER, &label)).size(text::LARGE).color(muted())
            } else {
                RichText::new(&label).size(text::LARGE).color(tokens::fg())
            };
            if level > 0 {
                ui.label(theme::icon_text(theme::icon::CHEVRON_RIGHT, theme::ICON_INLINE, muted()));
            }
            let siblings = if level == 0 { Vec::new() } else { children(&folders.all, &parent) };
            let here = parts[..level].join("/");
            let button = theme::chip_button(text, theme::CONTROL);
            egui::containers::menu::MenuButton::from_button(button)
                .ui(ui, |ui| {
                    theme::list_style(ui);
                    ui.set_min_width(260.0);
                    if !last {
                        let up = if level == 0 { t.directly_in_archive.to_string() } else { (t.file_here)(&label) };
                        if ui.button(up).on_hover_text(t.drop_deeper).clicked() {
                            chosen = Some(here.clone());
                            ui.close();
                        }
                    }
                    if !siblings.is_empty() {
                        ui.label(theme::field_label(t.instead));
                        ScrollArea::vertical().id_salt(("crumb", doc_id, level)).max_height(320.0).show(ui, |ui| {
                            for sib in &siblings {
                                let path = if parent.is_empty() { sib.clone() } else { format!("{parent}/{sib}") };
                                if ui.selectable_label(sib == &label, sib.as_str()).clicked() {
                                    chosen = Some(path);
                                    ui.close();
                                }
                            }
                        });
                    } else if level == 0 {
                        ui.label(theme::field_label(t.choose_area));
                        ScrollArea::vertical().id_salt(("crumb", doc_id, level)).max_height(320.0).show(ui, |ui| {
                            for top in children(&folders.all, "") {
                                if ui.selectable_label(parts.first() == Some(&top), top.as_str()).clicked() {
                                    chosen = Some(top.clone());
                                    ui.close();
                                }
                            }
                        });
                    }
                })
                .0
                .on_hover_text(t.change_folder);
        }
        // Subfolders of the target.
        let below = children(&folders.all, &form.folder);
        if !below.is_empty() {
            let label = RichText::new(t.add_subfolder).size(text::SMALL).color(muted());
            let button = theme::chip_button(label, theme::CONTROL);
            egui::containers::menu::MenuButton::from_button(button)
                .ui(ui, |ui| {
                    theme::list_style(ui);
                    ui.set_min_width(260.0);
                    ui.label(theme::field_label(t.subfolders));
                    ScrollArea::vertical().id_salt(("crumb-below", doc_id)).max_height(320.0).show(ui, |ui| {
                        for c in &below {
                            if ui.selectable_label(false, c.as_str()).clicked() {
                                let base = form.folder.trim_end_matches('/');
                                chosen = Some(if base.is_empty() { c.clone() } else { format!("{base}/{c}") });
                                ui.close();
                            }
                        }
                    });
                })
                .0
                .on_hover_text((t.subfolder_count)(below.len()));
        }
    });
    match chosen {
        Some(f) if f != form.folder => {
            form.folder = f;
            true
        }
        _ => false,
    }
}

/// A part of the file name template.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    Date,
    Doctype,
    Sender,
    Subject,
    Person,
    Extra,
}

impl Part {
    const ALL: [Part; 6] = [Part::Date, Part::Doctype, Part::Sender, Part::Subject, Part::Person, Part::Extra];

    fn bit(self) -> u8 {
        1 << self as u8
    }

    /// Fixed name, independent of the language.
    fn key(self) -> &'static str {
        match self {
            Part::Date => "date",
            Part::Doctype => "doctype",
            Part::Sender => "sender",
            Part::Subject => "subject",
            Part::Person => "person",
            Part::Extra => "extra",
        }
    }

    /// Label in the current language; so it is no good as an id (use `part as u8` for that).
    fn label(self) -> &'static str {
        let t = form_texts::t();
        match self {
            Part::Date => t.part_date,
            Part::Doctype => t.part_doctype,
            Part::Sender => t.part_sender,
            Part::Subject => t.part_subject,
            Part::Person => t.part_person,
            Part::Extra => t.part_extra,
        }
    }
}

/// Parts of a template in their order (`{year}`, `{month}`, `{day}` count as the date).
fn template_parts(template: &str) -> Vec<Part> {
    let mut found: Vec<(usize, Part)> = [
        ("{date}", Part::Date),
        ("{year}", Part::Date),
        ("{month}", Part::Date),
        ("{day}", Part::Date),
        ("{doctype}", Part::Doctype),
        ("{sender}", Part::Sender),
        ("{subject}", Part::Subject),
        ("{title}", Part::Subject),
        ("{person}", Part::Person),
        ("{extra}", Part::Extra),
    ]
    .into_iter()
    .filter_map(|(key, part)| template.find(key).map(|i| (i, part)))
    .collect();
    found.sort_by_key(|(i, _)| *i);
    let mut parts: Vec<Part> = Vec::new();
    for (_, part) in found {
        if !parts.contains(&part) {
            parts.push(part);
        }
    }
    parts
}

/// File name parts to click: each opens a small field to edit it, empty ones show as
/// "+ Person". The cross next to a part removes it from the name; a switched-off part shows
/// struck through, and a click adds it back. Returns whether the name changes.
/// Name parts. `own_name`: the name was just typed by hand or is the original name; the parts
/// then show faded, and changing one brings back the name built from the parts.
fn name_parts(ui: &mut egui::Ui, form: &mut Form, cfg: &Config, doc_id: u64, own_name: bool) -> bool {
    let template = analysis::folder_template(cfg, &form.folder).unwrap_or(&cfg.filename_template).clone();
    let t = form_texts::t();
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for part in template_parts(&template) {
            let value = match part {
                Part::Date => naming::format_date(analysis::parse_date(&form.date), &cfg.date_format),
                _ => naming::compact_component(form.value(part)),
            };
            let empty = value.is_empty() || (part == Part::Date && form.date.trim().is_empty());
            if !form.is_on(part) {
                let shown = if empty { part.label().to_string() } else { value };
                let text = RichText::new(shown).monospace().strikethrough().color(muted());
                let button = theme::chip_button(text, theme::COMPACT);
                if ui.add(button).on_hover_text((t.part_back_in)(part.label())).clicked() {
                    form.off &= !part.bit();
                    changed = true;
                }
                continue;
            }
            let text = if empty {
                RichText::new(format!("+ {}", part.label())).size(text::SMALL).color(muted())
            } else {
                RichText::new(value).monospace().color(if own_name { muted() } else { tokens::fg() })
            };
            // Part and cross stay together: if they no longer fit in the row, a new row starts.
            let width = text_width(ui, text.clone()) + 2.0 * ui.spacing().button_padding.x + 24.0;
            if width > ui.available_size_before_wrap().x {
                ui.end_row();
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let button = theme::chip_button(text, theme::COMPACT);
                egui::containers::menu::MenuButton::from_button(button)
                    .ui(ui, |ui| {
                        theme::list_style(ui);
                        ui.set_min_width(280.0);
                        changed |= part_editor(ui, part, form, cfg, doc_id);
                    })
                    .0
                    .on_hover_text(match own_name {
                        true => (t.part_change_rebuild)(part.label()),
                        false => (t.part_change)(part.label()),
                    });
                if !empty {
                    let x = egui::Button::new(theme::icon_text(theme::icon::X, 12.0, muted()))
                        .frame(false)
                        .min_size(Vec2::new(20.0, 24.0));
                    if ui.add(x).on_hover_text((t.part_remove)(part.label())).clicked() {
                        form.off |= part.bit();
                        changed = true;
                    }
                }
            });
        }
    });
    changed
}

/// Field for a part; for doctype, person and extra with a drop-down list.
fn part_editor(ui: &mut egui::Ui, part: Part, form: &mut Form, cfg: &Config, doc_id: u64) -> bool {
    let t = form_texts::t();
    ui.label(theme::field_label(part.label()));
    let (value, hint, choices): (&mut String, &str, Vec<String>) = match part {
        Part::Date => (&mut form.date, t.hint_date, Vec::new()),
        Part::Doctype => (&mut form.doctype, t.hint_doctype, cfg.doctypes.clone()),
        Part::Sender => (&mut form.sender, t.hint_sender, Vec::new()),
        Part::Subject => (&mut form.subject, t.hint_subject, Vec::new()),
        Part::Person => {
            let mut list: Vec<String> = cfg.persons.iter().map(|p| p.name.clone()).collect();
            if cfg.persons.len() > 1 {
                list.push("Familie".into());
            }
            (&mut form.person, t.hint_optional, list)
        }
        Part::Extra => (&mut form.extra, t.hint_optional, cfg.extras.clone()),
    };
    let edit = theme::field(ui, TextEdit::singleline(value).hint_text(hint).desired_width(f32::INFINITY));
    if edit.gained_focus() || !ui.memory(|m| m.focused().is_some()) {
        edit.request_focus();
    }
    let mut changed = edit.changed();
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        ui.close();
    }
    if !choices.is_empty() {
        let needle = archive::normalize(value.trim());
        let filtered: Vec<&String> = choices
            .iter()
            .filter(|c| needle.is_empty() || archive::normalize(c).contains(&needle) || part != Part::Doctype)
            .collect();
        ScrollArea::vertical().id_salt(("part-choices", doc_id, part as u8)).max_height(260.0).show(ui, |ui| {
            if part != Part::Doctype && ui.selectable_label(value.is_empty(), t.no_choice).clicked() {
                value.clear();
                changed = true;
                ui.close();
            }
            for c in filtered {
                if ui.selectable_label(value == c, c.as_str()).clicked() {
                    *value = c.clone();
                    changed = true;
                    ui.close();
                }
            }
        });
    }
    if part == Part::Date && !value.trim().is_empty() && analysis::parse_date(value).is_none() {
        ui.label(RichText::new(t.date_unrecognized).small().color(status_err()));
    }
    changed
}

/// Existing folders similar to a target that does not exist (typos, different level).
fn similar_folders(target: &str, folders: &[String]) -> Vec<String> {
    let want = archive::normalize(&archive::split_folder(target).join("/"));
    let want_last = want.rsplit('/').next().unwrap_or(&want).to_string();
    if want_last.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(usize, &String)> = folders
        .iter()
        .filter_map(|f| {
            let have = archive::normalize(f);
            let have_last = have.rsplit('/').next().unwrap_or(&have);
            let whole = edit_distance(&have, &want);
            let last = edit_distance(have_last, &want_last);
            let limit = (want.chars().count() / 8).max(2);
            let score = if have_last == want_last { 1 } else { whole.min(last + 1) };
            (score <= limit).then_some((score, f))
        })
        .collect();
    scored.sort_by_key(|(score, f)| (*score, f.len()));
    scored.into_iter().take(3).map(|(_, f)| f.clone()).collect()
}

/// Number of insertions, deletions and substitutions from `a` to `b` (Levenshtein).
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// Folder search above the path. The hits (by name or description) appear in a list above the field,
/// Enter takes the first, Esc clears the field. Returns whether a folder was chosen.
fn folder_search_field(
    ui: &mut egui::Ui,
    folder: &mut String,
    folders: &FolderIndex,
    cfg: &Config,
    filter: &mut String,
    doc_id: u64,
) -> bool {
    let resp = theme::field(
        ui,
        TextEdit::singleline(filter)
            .hint_text(theme::with_icon(theme::icon::SEARCH, form_texts::t().folder_search))
            .desired_width(f32::INFINITY),
    );
    let popup_id = ui.make_persistent_id(("folder-search", doc_id));
    let typed = !filter.trim().is_empty();
    if resp.has_focus() && typed {
        egui::Popup::open_id(ui.ctx(), popup_id);
    } else if !typed && egui::Popup::is_id_open(ui.ctx(), popup_id) {
        egui::Popup::close_id(ui.ctx(), popup_id);
    }
    let mut changed = false;
    if resp.lost_focus() {
        if ui.input(|i| i.key_pressed(egui::Key::Enter))
            && let Some(first) = folder_hits(folders, cfg, filter).first()
        {
            *folder = (*first).clone();
            changed = true;
        } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            filter.clear();
        }
    }
    // The filing target sits at the bottom of the window: hits go above it so they stay inside the window.
    egui::Popup::from_response(&resp)
        .id(popup_id)
        .open_memory(None)
        .align(egui::RectAlign::TOP_START)
        .align_alternatives(&[])
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            theme::list_style(ui);
            ui.set_min_width(resp.rect.width().min(560.0));
            changed |= folder_search_results(ui, folder, folders, cfg, filter);
        });
    if changed {
        filter.clear();
        egui::Popup::close_id(ui.ctx(), popup_id);
    }
    changed
}

/// Folders whose name or description contains all search words.
fn folder_hits<'a>(folders: &'a FolderIndex, cfg: &Config, filter: &str) -> Vec<&'a String> {
    let needle = archive::normalize(filter.trim());
    let words: Vec<&str> = needle.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    folders
        .all
        .iter()
        .filter(|f| {
            let n = archive::normalize(f);
            let note = analysis::folder_note(&cfg.folder_notes, f).map(archive::normalize).unwrap_or_default();
            words.iter().all(|w| n.contains(w) || note.contains(w))
        })
        .collect()
}

/// Hit list of the folder search. If the searched name does not exist, it can be taken as a new subfolder
/// of the current target. Returns whether a folder was chosen.
fn folder_search_results(
    ui: &mut egui::Ui,
    folder: &mut String,
    folders: &FolderIndex,
    cfg: &Config,
    filter: &str,
) -> bool {
    let t = form_texts::t();
    let hits = folder_hits(folders, cfg, filter);
    if folders.all.is_empty() {
        ui.label(RichText::new(t.no_folders).color(muted()));
    } else if hits.is_empty() {
        ui.label(RichText::new(t.no_match).color(muted()));
    }
    let needle = archive::normalize(filter.trim());
    let name = filter.trim().to_string();
    let exact = folders.all.iter().any(|f| archive::normalize(f.rsplit('/').next().unwrap_or(f)) == needle);
    if !exact && !name.contains('/') && archive::safe_relative_folder(&name).is_ok() {
        let base = folder.trim_end_matches('/').to_string();
        let new = if base.is_empty() { name.clone() } else { format!("{base}/{name}") };
        let label = theme::with_icon(theme::icon::FOLDER_PLUS, &(t.new_subfolder)(&new.replace('/', " › ")));
        if ui.add(egui::Button::new(RichText::new(label).color(accent())).frame(false)).clicked() {
            *folder = new;
            return true;
        }
    }
    let mut changed = false;
    ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
        for f in hits {
            // The whole path, otherwise folders with the same name ("2026") would look alike.
            let mut label = RichText::new(f.replace('/', " › "));
            if !f.contains('/') {
                label = label.family(theme::semibold());
            }
            let resp = ui.selectable_label(*folder == *f, label);
            let resp = match analysis::folder_note(&cfg.folder_notes, f) {
                Some(note) => resp.on_hover_text(format!("{f}\n{note}")),
                None => resp.on_hover_text(f.as_str()),
            };
            if resp.clicked() {
                *folder = f.clone();
                changed = true;
            }
        }
    });
    changed
}

/// Preview: page images with paging and zoom, or the content for text and Office files.
pub struct Preview<'a> {
    /// All pages of the document; the app loads missing ones while they are shown.
    pub pages: &'a PageCache,
    pub page: &'a mut usize,
    /// 1 = whole page; larger = zoomed in with scroll bars.
    pub zoom: &'a mut f32,
    /// Rotation chosen by hand per page (clockwise); the buttons change it, the page is drawn that way.
    pub turns: &'a mut Vec<u16>,
    pub path: &'a Path,
    pub analysis: Option<&'a Analysis>,
}

/// Height of the bar above the page when it fits in one row.
const BAR_HEIGHT: f32 = 36.0;

/// Real size: the longer side measures 29.7 cm as with DIN A4, at 96 dots per inch. Scrinium does not know
/// the paper size itself; pages arrive as images.
const REAL_LONG_SIDE: f32 = 297.0 / 25.4 * 96.0;

/// Next zoom step in percent of the real size, up or down from `pct`.
fn zoom_step(pct: f32, up: bool) -> f32 {
    const STEPS: [f32; 11] = [25.0, 33.0, 50.0, 67.0, 75.0, 100.0, 125.0, 150.0, 200.0, 300.0, 400.0];
    if up {
        STEPS.iter().copied().find(|s| *s > pct + 0.5).unwrap_or(STEPS[STEPS.len() - 1])
    } else {
        STEPS.iter().rev().copied().find(|s| *s < pct - 0.5).unwrap_or(STEPS[0])
    }
}

/// Flag whether a document's preview shows the recognized text instead of the page.
fn text_view_id(path: &Path) -> egui::Id {
    egui::Id::unique(("show-text", path))
}

/// Page size while none is loaded: A4 portrait like the preview images.
const PLACEHOLDER_PAGE: Vec2 = Vec2::new(990.0, 1400.0);

pub fn preview_ui(ui: &mut egui::Ui, view: Preview, size: Vec2) {
    let Preview { pages, page, zoom, turns, path, analysis } = view;
    if turns.len() < pages.count() {
        turns.resize(pages.count(), 0);
    }
    let t = form_texts::t();
    let g = general::t();
    // The bar above the page wraps in narrow windows (multiple pages, with text). Its height in the last
    // frame decides the room for the page below.
    let bar_id = ui.scope_id().with("preview-bar");
    let nav_h: f32 = ui.data(|d| d.get_temp(bar_id)).unwrap_or(BAR_HEIGHT).max(BAR_HEIGHT);
    let has_text = analysis.is_some_and(|a| !a.text.trim().is_empty());
    let text_id = text_view_id(path);
    let mut show_text: bool = has_text && ui.data(|d| d.get_temp(text_id)).unwrap_or(false);
    // Page size: `fit` fits it entirely (zoom 1), at zoom `real` it appears at real size.
    let max = Vec2::new((size.x - 16.0).max(100.0), (size.y - nav_h - 24.0).max(100.0));
    let current = (*page).min(pages.count().saturating_sub(1));
    let quarter = turns.get(current).map_or(0, |t| t / 90 % 4);
    // While the page loads, a loaded page nearby lends its size, so the zoom does not jump when paging.
    let full = (!pages.is_empty())
        .then(|| turned_size(pages.any_loaded(current).map_or(PLACEHOLDER_PAGE, |t| t.size_vec2()), quarter));
    let fit = full.map_or(1.0, |f| (max.x / f.x).min(max.y / f.y));
    let real = full.map_or(1.0, |f| REAL_LONG_SIDE / (f.max_elem() * fit));
    let (lo, hi) = (real.min(1.0), (4.0 * real).max(1.0));
    *zoom = zoom.clamp(lo, hi);
    let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
    // Groups only wrap as a whole: a new row starts before each group that no longer fits.
    let icon = theme::ARROW_BUTTON;
    let gap = ui.spacing().item_spacing.x;
    let bar = ui.horizontal_wrapped(|ui| {
        if !pages.is_empty() && has_text {
            let text_label = theme::with_icon(theme::icon::FILE_TEXT, t.text);
            wrap_for(ui, chip_width(ui, t.page) + chip_width(ui, &text_label) + SEGMENT_GROUP);
            let before = show_text;
            theme::segments(ui, theme::CONTROL, |ui| {
                theme::segment_value(ui, &mut show_text, false, t.page);
                theme::segment_value(ui, &mut show_text, true, text_label).on_hover_text(t.recognized_text);
            });
            if show_text != before {
                ui.data_mut(|d| d.insert_temp(text_id, show_text));
            }
            ui.add_space(8.0);
        }
        if !show_text && pages.count() > 1 {
            let pages = pages.count();
            *page = (*page).min(pages - 1);
            let label = (t.page_of)(*page + 1, pages);
            wrap_for(ui, 2.0 * (icon + gap) + text_width(ui, RichText::new(&label)));
            if theme::arrow_button(ui, Arrow::Left, *page > 0, &(t.prev_page)(g.key_page_up)) {
                *page -= 1;
            }
            ui.label(label);
            if theme::arrow_button(ui, Arrow::Right, *page + 1 < pages, &(t.next_page)(g.key_page_down)) {
                *page += 1;
            }
            ui.add_space(8.0);
        }
        if !show_text && !pages.is_empty() {
            let pct = *zoom / real * 100.0;
            let wheel = keys(&[g.key_ctrl, g.key_wheel]);
            wrap_for(ui, 2.0 * (icon + gap) + ZOOM_LABEL_WIDTH);
            if theme::icon_button(ui, theme::icon::ZOOM_OUT, *zoom > lo + 1e-3, &(t.zoom_out)(&wheel)) {
                *zoom = (zoom_step(pct, false) / 100.0 * real).clamp(lo, hi);
            }
            // Fixed width so "+" does not jump when the number changes.
            let percent = egui::Label::new(RichText::new((t.percent)(pct)).small().color(muted()));
            ui.add_sized([ZOOM_LABEL_WIDTH, 20.0], percent);
            if theme::icon_button(ui, theme::icon::ZOOM_IN, *zoom < hi - 1e-3, &(t.zoom_in)(&wheel)) {
                *zoom = (zoom_step(pct, true) / 100.0 * real).clamp(lo, hi);
            }
            ui.add_space(4.0);
            let hundred = (t.percent)(100.0);
            wrap_for(ui, chip_width(ui, t.whole_page) + chip_width(ui, &hundred) + SEGMENT_GROUP);
            theme::segments(ui, theme::CONTROL, |ui| {
                let whole = theme::segment(ui, near(*zoom, 1.0), t.whole_page).on_hover_text(t.whole_page_hover);
                if whole.clicked() {
                    *zoom = 1.0;
                }
                if theme::segment(ui, near(*zoom, real), hundred).on_hover_text(t.real_size_hover).clicked() {
                    *zoom = real;
                }
            });
            ui.add_space(8.0);
            wrap_for(ui, 2.0 * icon + gap);
            if theme::icon_button(ui, theme::icon::ROTATE_CCW, true, t.turn_left) {
                turns[current] = (turns[current] + 270) % 360;
            }
            if theme::icon_button(ui, theme::icon::ROTATE_CW, true, t.turn_right) {
                turns[current] = (turns[current] + 90) % 360;
            }
        }
        // At the far right while it fits in the row; otherwise at the start of the next.
        if ui.available_size_before_wrap().x < theme::ARROW_BUTTON + 8.0 {
            ui.end_row();
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::ghost_button(ui, &theme::icon::EXTERNAL.to_string()).on_hover_text(t.open_original).clicked() {
                crate::settings::open_path(path);
            }
        });
    });
    let bar_h = bar.response.rect.height() + 4.0;
    if (bar_h - nav_h).abs() > 0.5 {
        ui.data_mut(|d| d.insert_temp(bar_id, bar_h));
        ui.ctx().request_repaint();
    }
    if pages.is_empty() || show_text {
        let Some(a) = analysis else { return };
        egui::Frame::new()
            .fill(tokens::surface())
            .stroke(Stroke::new(1.0_f32, tokens::line()))
            .corner_radius(theme::radius::LG)
            .inner_margin(Margin::same(16))
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(size.x - 34.0, (size.y - nav_h - 34.0).max(120.0)));
                ui.set_max_width(size.x - 34.0);
                let head =
                    if show_text { (t.recognized_text_head)(a.kind.label()) } else { a.kind.label().to_string() };
                ui.label(theme::field_label(&head));
                ui.add_space(8.0);
                if a.text.trim().is_empty() {
                    ui.label(RichText::new(t.no_content).color(muted()));
                } else {
                    // Normal font with line wrapping: reads better than a monospace font.
                    ScrollArea::vertical().id_salt(("text-preview", path)).max_height(size.y - nav_h - 70.0).show(
                        ui,
                        |ui| {
                            let shown: String = a.text.chars().take(20_000).collect();
                            ui.add(egui::Label::new(RichText::new(shown).size(text::BODY)).wrap());
                        },
                    );
                }
            });
        return;
    }
    // Neutral passe-partout: the page takes no color cast from the UI.
    let passepartout = egui::Frame::new().fill(tokens::canvas()).corner_radius(theme::radius::LG);
    let tex = match pages.get(current) {
        Some(Slot::Loaded(tex)) => tex,
        other => {
            // The page is still loading or failed to load: an empty sheet of page size in its place.
            let shown = (full.unwrap_or(PLACEHOLDER_PAGE) * fit).min(max);
            passepartout.inner_margin(Margin::same(8)).show(ui, |ui| {
                let sheet = egui::Frame::new().fill(tokens::surface()).stroke(Stroke::new(1.0_f32, tokens::line()));
                sheet.show(ui, |ui| {
                    let (rect, _) = ui.allocate_exact_size(shown, egui::Sense::hover());
                    let inner =
                        egui::UiBuilder::new().max_rect(rect.shrink(16.0)).layout(Layout::top_down(Align::Center));
                    let mut ui = ui.new_child(inner);
                    ui.add_space(rect.height() * 0.4);
                    match other {
                        Some(Slot::Failed(e)) => {
                            ui.add(egui::Label::new(RichText::new((t.page_failed)(e)).color(status_err())).wrap());
                        }
                        _ => {
                            ui.add(egui::Spinner::new().size(24.0).color(muted()));
                            ui.label(RichText::new(t.page_loading).color(muted()));
                        }
                    }
                });
            });
            return;
        }
    };
    let shown = turned_size(tex.size_vec2(), quarter) * fit * *zoom;
    passepartout.inner_margin(Margin::same(8)).show(ui, |ui| {
        let resp =
            ScrollArea::both().id_salt("page-zoom").max_width(max.x).max_height(max.y).auto_shrink([true, true]).show(
                ui,
                |ui| {
                    egui::Frame::new()
                        .stroke(Stroke::new(1.0_f32, tokens::line()))
                        .shadow(tokens::shadow())
                        .show(ui, |ui| {
                            let (rect, resp) = ui.allocate_exact_size(shown, egui::Sense::CLICK);
                            paint_turned(ui.painter(), rect, tex.id(), quarter);
                            // Click: from whole page to real size (or double, if it already is that large).
                            let target = if real > 1.0 + 1e-3 { real } else { 2.0_f32.min(hi) };
                            let fitted = near(*zoom, 1.0);
                            let hover = if fitted { t.click_zoom_in } else { t.click_whole_page };
                            let resp = resp.on_hover_cursor(egui::CursorIcon::ZoomIn).on_hover_text(hover);
                            if resp.clicked() {
                                *zoom = if fitted { target } else { 1.0 };
                            }
                            resp.hovered()
                        })
                        .inner
                },
            );
        // Ctrl+mouse wheel over the page: zoom in and out.
        let delta = ui.input(|i| i.zoom_delta());
        if resp.inner && delta != 1.0 {
            *zoom = (*zoom * delta).clamp(lo, hi);
        }
    });
}

/// Width of the zoom display ("55 %") in the bar above the page.
const ZOOM_LABEL_WIDTH: f32 = 46.0;

/// Starts a new row in the wrapping bar if a group of this width no longer fits.
fn wrap_for(ui: &mut egui::Ui, width: f32) {
    if width > ui.available_size_before_wrap().x {
        ui.end_row();
    }
}

/// What a segment group adds to its segments: border, inner padding and gap.
const SEGMENT_GROUP: f32 = 2.0 * 3.0 + 2.0;

/// Width of a segment (`theme::segment`) with this text.
fn chip_width(ui: &egui::Ui, text: &str) -> f32 {
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    galley.size().x + 2.0 * theme::space::S3
}

/// Size of a page after `quarter` quarter turns.
fn turned_size(size: Vec2, quarter: u16) -> Vec2 {
    if quarter % 2 == 1 { Vec2::new(size.y, size.x) } else { size }
}

/// Draws the page rotated clockwise by `quarter` quarter turns into `rect`: the corners of the rectangle
/// get the image corners, shifted by that many places.
fn paint_turned(painter: &egui::Painter, rect: egui::Rect, texture: egui::TextureId, quarter: u16) {
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
    let uv = [egui::pos2(0.0, 0.0), egui::pos2(1.0, 0.0), egui::pos2(1.0, 1.0), egui::pos2(0.0, 1.0)];
    let mut mesh = egui::Mesh::with_texture(texture);
    for (i, pos) in corners.into_iter().enumerate() {
        let uv = uv[(i + 4 - quarter as usize % 4) % 4];
        mesh.vertices.push(egui::epaint::Vertex { pos, uv, color: Color32::WHITE });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn zoom_steps_land_on_whole_percentages() {
        assert_eq!(zoom_step(56.0, true), 67.0);
        assert_eq!(zoom_step(56.0, false), 50.0);
        assert_eq!(zoom_step(100.0, true), 125.0, "onward from 100 %, not back to 100 %");
        assert_eq!(zoom_step(100.0, false), 75.0);
        assert_eq!(zoom_step(400.0, true), 400.0);
        assert_eq!(zoom_step(20.0, false), 25.0);
    }

    #[test]
    fn folder_helpers() {
        let f = list(&["Bank", "Bank/Depot", "Bank/2026", "Haus", "Haus/_Strom", "Haus/_Strom/2026"]);
        assert_eq!(children(&f, ""), ["Bank", "Haus"]);
        assert_eq!(children(&f, "bank"), ["Depot", "2026"]);
        assert_eq!(children(&f, "Haus/_Strom"), ["2026"]);
        assert!(children(&f, "Haus/_Strom/2026").is_empty());
        assert_eq!(similar_folders("Bank/Dpot", &f), ["Bank/Depot"], "typo");
        assert_eq!(similar_folders("Versicherung/Depot", &f).first().map(String::as_str), Some("Bank/Depot"));
        assert!(similar_folders("Ganz/Anders", &f).is_empty());
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn parts_switched_off_leave_the_name() {
        let cfg = Config { archive_root: std::env::temp_dir().display().to_string(), ..Default::default() };
        let mut form = Form {
            date: "2026-04-15".into(),
            doctype: "Rechnung".into(),
            sender: "Autohaus-Muster".into(),
            subject: "Bremsen".into(),
            ..Default::default()
        };
        assert_eq!(auto_filename(&cfg, &form, false, ""), "260415_Rechnung_Autohaus-Muster_Bremsen");
        form.off |= Part::Date.bit() | Part::Sender.bit();
        assert_eq!(auto_filename(&cfg, &form, false, ""), "Rechnung_Bremsen", "no double separators");
        assert_eq!(form.date, "2026-04-15", "the value stays");
        form.off |= Part::Doctype.bit();
        assert_eq!(auto_filename(&cfg, &form, false, ""), "Bremsen");
        // A switched-off date does not block filing, even if it cannot be read.
        form.filename = "Bremsen".into();
        form.date = "irgendwann".into();
        assert_eq!(file_blocker(&cfg, &form), None);
        form.off = 0;
        assert_eq!(file_blocker(&cfg, &form).as_deref(), Some("Datum nicht erkannt"));
        form.off = Part::Person.bit();
        form.reset_name(&analysis::Suggestion::default());
        assert_eq!(form.off, 0, "resetting to the suggestion switches all parts back on");
    }

    #[test]
    fn template_parts_in_order() {
        let parts = template_parts("{date}_{doctype}_{sender}_{subject}_{person}_{extra}");
        assert_eq!(parts, [Part::Date, Part::Doctype, Part::Sender, Part::Subject, Part::Person, Part::Extra]);
        assert_eq!(template_parts("{sender}_{title}_{year}-{month}"), [Part::Sender, Part::Subject, Part::Date]);
    }
}
