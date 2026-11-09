//! Settings in tabs: archive, file names, special rules, AI and – set apart – set up with AI.
//! Each tab shows the important things first; technical details are collapsed under *Advanced*.

use std::path::Path;

use eframe::egui::{self, Align, Layout, Margin, RichText, ScrollArea, Sense, TextEdit, Vec2};

use crate::about;
use crate::archive::{self, FolderIndex};
use crate::config::{Alias, Config, FolderTemplate, LlmBackend, OcrConfig, OcrQuality, Rule};
use crate::folder_view::FolderView;
use crate::i18n::Lang;
use crate::i18n::settings::t;
use crate::llm::LlmClient;
use crate::llm_ui::LocalAi;
use crate::naming::{self, DocDate, NameRules, TemplateFields};
use crate::schema;
use crate::setup;
use crate::theme::{self, Arrow, text, tokens};
use tokens::{muted, status_err, status_ok, status_warn};

/// Minimum width of the tab bar; with Segoe UI the longest tab name (Spanish "Configurar con IA" with
/// icon) fits with room to spare.
const TAB_BAR_MIN_WIDTH: f32 = 208.0;
/// Inner padding of bar and tab, 12 px each on the left and right.
const TAB_BAR_PADDING: f32 = 2.0 * 12.0 + 2.0 * theme::space::S3;

/// Width of the tab bar: at least [`TAB_BAR_MIN_WIDTH`], wider if the longest tab name in semibold with 5 %
/// margin (§10) would not fit otherwise – on Linux, DejaVu Sans Bold is much wider than Segoe UI. The width
/// depends only on language and font, so it does not jump when switching tabs (R1).
fn tab_bar_width(ctx: &egui::Context, labels: &[&str]) -> f32 {
    let font = egui::FontId::new(text::LARGE, theme::semibold());
    let widest = ctx.fonts_mut(|f| {
        labels
            .iter()
            .map(|l| f.layout_no_wrap(l.to_string(), font.clone(), egui::Color32::WHITE).size().x)
            .fold(0.0, f32::max)
    });
    (widest * 1.05 + TAB_BAR_PADDING).ceil().max(TAB_BAR_MIN_WIDTH)
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    General,
    Scan,
    Archive,
    Names,
    Rules,
    Ai,
    Setup,
}

impl Tab {
    /// Tabs for the settings themselves, in the order of a document's path: general, scanning,
    /// archive and name, special cases, the AI last. *Set up with AI* sits apart below them.
    const MAIN: [Tab; 6] = [Tab::General, Tab::Scan, Tab::Archive, Tab::Names, Tab::Rules, Tab::Ai];

    /// All tabs in list order, for Ctrl+Tab.
    const ORDER: [Tab; 7] = [Tab::General, Tab::Scan, Tab::Archive, Tab::Names, Tab::Rules, Tab::Ai, Tab::Setup];

    /// The tab `step` places further in the list, wrapping around (−1 = the previous one).
    fn step(self, step: isize) -> Tab {
        let len = Tab::ORDER.len() as isize;
        let i = Tab::ORDER.iter().position(|&t| t == self).unwrap_or(0) as isize;
        Tab::ORDER[(i + step).rem_euclid(len) as usize]
    }

    /// Name in the tab list.
    fn label(self) -> &'static str {
        let t = t();
        match self {
            Tab::General => t.tab_general,
            Tab::Scan => t.tab_scan,
            Tab::Archive => t.tab_archive,
            Tab::Names => t.tab_names,
            Tab::Rules => t.tab_rules,
            Tab::Ai => t.tab_ai,
            Tab::Setup => t.setup_with_ai,
        }
    }
}

pub struct RuleDraft {
    name: String,
    enabled: bool,
    keywords: String,
    exclude: String,
    require: String,
    match_all: bool,
    folder: String,
    doctype: String,
    sender: String,
    person: String,
}

impl RuleDraft {
    fn empty() -> Self {
        Self {
            name: String::new(),
            enabled: true,
            keywords: String::new(),
            exclude: String::new(),
            require: String::new(),
            match_all: false,
            folder: String::new(),
            doctype: String::new(),
            sender: String::new(),
            person: String::new(),
        }
    }
}

/// Editing state; lists are edited as text (one line per entry).
pub struct SettingsDraft {
    pub config: Config,
    pub tab: Tab,
    rules: Vec<RuleDraft>,
    /// Folder overview: used, descriptions, fallback folder.
    folders: FolderView,
    doctypes: String,
    extras: String,
    persons: String,
    senders: String,
    keep_names: String,
    folder_templates: String,
    /// Rule whose fields are currently expanded.
    editing_rule: Option<usize>,
    /// Text the rules are tried on.
    rule_probe: String,
    test_result: Option<Result<Vec<String>, String>>,
    /// Running connection test.
    test_rx: Option<std::sync::mpsc::Receiver<Result<Vec<String>, String>>>,
    /// State on opening (as `build()` returns it): if the draft differs from it, there are unsaved changes.
    baseline: Config,
    /// Modification time of the config file on opening; if it changes before saving, someone
    /// edited the file elsewhere.
    pub opened: Option<std::time::SystemTime>,
    /// Pending question: there are unsaved changes, and the view is about to be left.
    pub leave: Option<Leave>,
    /// The file was changed elsewhere since opening: overwrite or reload?
    pub conflict: bool,
    /// Error from the last save; the draft stays open.
    pub save_error: Option<String>,
}

/// Where to go when leaving the view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Leave {
    /// Back to the documents.
    Back,
    /// Reload the config file.
    Reload,
    /// Quit Scrinium (close the window).
    Quit,
}

pub enum SettingsAction {
    /// From the *General* tab; takes effect at once and is not part of the draft.
    General(about::GeneralAction),
    Save,
    /// Save even if the file was changed elsewhere in the meantime.
    SaveAnyway,
    /// Save, then quit Scrinium.
    SaveAndQuit,
    /// Back to the documents, discarding changes.
    Cancel,
    Reload,
    /// Quit Scrinium, discarding changes.
    Quit,
    /// Have the settings generated from the description in the *Set up with AI* tab.
    GenerateSetup,
}

/// AI state for the header of the AI tab: ready? and one line of text.
pub struct AiStatus {
    pub ready: bool,
    pub text: String,
}

fn lines(v: &[String]) -> String {
    v.join("\n")
}

fn from_lines(s: &str) -> Vec<String> {
    s.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
}

fn alias_lines(v: &[Alias]) -> String {
    v.iter()
        .map(|a| if a.aliases.is_empty() { a.name.clone() } else { format!("{}: {}", a.name, a.aliases.join(", ")) })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `Max: Maximilian, MM` → name with spellings.
fn aliases_from_lines(s: &str) -> Vec<Alias> {
    s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| match l.split_once(':') {
            Some((name, rest)) => Alias {
                name: name.trim().to_string(),
                aliases: rest.split(',').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect(),
            },
            None => Alias { name: l.to_string(), aliases: Vec::new() },
        })
        .filter(|a| !a.name.is_empty())
        .collect()
}

fn pair_lines<'a>(pairs: impl Iterator<Item = (&'a String, &'a String)>) -> String {
    pairs.map(|(k, v)| format!("{k} = {v}")).collect::<Vec<_>>().join("\n")
}

fn pairs_from_lines(s: &str) -> Vec<(String, String)> {
    s.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .filter(|(k, v)| !k.is_empty() && !v.is_empty())
        .collect()
}

impl SettingsDraft {
    pub fn new(config: &Config, tab: Tab) -> Self {
        let rules = config
            .rules
            .iter()
            .map(|r| RuleDraft {
                name: r.name.clone(),
                enabled: r.enabled,
                keywords: r.keywords.join(", "),
                exclude: r.exclude_keywords.join(", "),
                require: r.require_keywords.join(", "),
                match_all: r.match_all,
                folder: r.folder.clone(),
                doctype: r.doctype.clone().unwrap_or_default(),
                sender: r.sender.clone().unwrap_or_default(),
                person: r.person.clone().unwrap_or_default(),
            })
            .collect();
        Self {
            config: config.clone(),
            tab,
            rules,
            folders: FolderView::new(config),
            doctypes: lines(&config.doctypes),
            extras: lines(&config.extras),
            persons: alias_lines(&config.persons),
            senders: alias_lines(&config.senders),
            keep_names: config.keep_name_extensions.join(", "),
            folder_templates: pair_lines(config.folder_templates.iter().map(|t| (&t.folder, &t.template))),
            editing_rule: None,
            rule_probe: String::new(),
            test_result: None,
            test_rx: None,
            baseline: config.clone(),
            opened: None,
            leave: None,
            conflict: false,
            save_error: None,
        }
        .with_baseline()
    }

    /// Remembers the state on opening, as `build()` returns it (trimmed and sorted).
    fn with_baseline(mut self) -> Self {
        self.baseline = self.build();
        self
    }

    /// New draft from `cfg` (e.g. after *Apply*); the baseline stays, so whatever was applied
    /// counts as unsaved.
    /// The view (scanned folders, expanded items, search) stays as it was.
    pub fn rebuilt(&mut self, cfg: &Config) -> Self {
        let mut next = Self::new(cfg, self.tab);
        next.baseline = self.baseline.clone();
        next.opened = self.opened;
        next.rule_probe = self.rule_probe.clone();
        next.folders.keep_view(&mut self.folders);
        next
    }

    /// Are there changes that are not saved yet?
    pub fn is_dirty(&self) -> bool {
        self.build() != self.baseline
    }

    /// Asks to leave: at once without changes, otherwise only after a confirmation question.
    pub fn request_leave(&mut self, leave: Leave) -> Option<SettingsAction> {
        if self.is_dirty() {
            self.leave = Some(leave);
            return None;
        }
        Some(match leave {
            Leave::Back => SettingsAction::Cancel,
            Leave::Reload => SettingsAction::Reload,
            Leave::Quit => SettingsAction::Quit,
        })
    }

    /// Configuration with all edited lists.
    pub fn build(&self) -> Config {
        let mut cfg = self.config.clone();
        cfg.rules = rules_from_drafts(&self.rules);
        self.folders.apply(&mut cfg);
        cfg.doctypes = from_lines(&self.doctypes);
        cfg.extras = from_lines(&self.extras);
        cfg.persons = aliases_from_lines(&self.persons);
        cfg.senders = aliases_from_lines(&self.senders);
        cfg.keep_name_extensions =
            self.keep_names.split([',', '\n']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        cfg.folder_templates = pairs_from_lines(&self.folder_templates)
            .into_iter()
            .map(|(folder, template)| FolderTemplate { folder, template })
            .collect();
        cfg
    }

    pub fn set_probe(&mut self, text: &str) {
        self.rule_probe = text.chars().take(4000).collect();
    }
}

fn rules_from_drafts(drafts: &[RuleDraft]) -> Vec<Rule> {
    let list = |s: &str| s.split(',').map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect::<Vec<_>>();
    let opt = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
    drafts
        .iter()
        .filter(|r| !r.keywords.trim().is_empty() && !r.folder.trim().is_empty())
        .map(|r| Rule {
            name: r.name.trim().to_string(),
            enabled: r.enabled,
            keywords: list(&r.keywords),
            match_all: r.match_all,
            exclude_keywords: list(&r.exclude),
            require_keywords: list(&r.require),
            folder: r.folder.trim().to_string(),
            doctype: opt(&r.doctype),
            sender: opt(&r.sender),
            person: opt(&r.person),
        })
        .collect()
}

fn hint(ui: &mut egui::Ui, s: &str) {
    ui.label(RichText::new(s).small().color(muted()));
}

/// Multi-line text field for a list (one line per entry), with its own scroll bar.
fn list_box(ui: &mut egui::Ui, id: &str, value: &mut String, rows: usize) {
    let height = (rows.max(3) as f32) * 19.0 + 12.0;
    ScrollArea::vertical().id_salt(("list", id)).max_height(height).show(ui, |ui| {
        theme::text_area(
            ui,
            TextEdit::multiline(value).desired_rows(rows).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace),
        );
    });
}

/// Collapsed list with the count in the title, e.g. "Doctypes (52)". `id` stays the same in every language
/// so that expanded sections stay open when the language changes.
fn list_section(ui: &mut egui::Ui, id: &str, title: &str, value: &mut String, rows: usize, help: &str) {
    let n = from_lines(value).len();
    theme::collapsing(RichText::new(format!("{title} ({n})")).size(text::BODY)).id_salt(("list-section", id)).show(
        ui,
        |ui| {
            hint(ui, help);
            list_box(ui, id, value, rows);
        },
    );
}

/// What the settings need from the app that is not in the draft: the AI state and, for the
/// *General* tab, appearance and updates.
pub struct AppState<'a> {
    pub ai: &'a AiStatus,
    pub mode: theme::Mode,
    pub updates: &'a crate::update::Updates,
}

/// Maximum content width – beyond it, lines get too long to read.
const CONTENT_MAX_WIDTH: f32 = 1400.0;

/// Settings as a separate view in the main window: tabs on the left, content on the right over the full
/// height, Save and Cancel at the bottom. Returns the chosen action (save, cancel, reload, set up).
/// Before the call only the bars at the top and bottom are placed; this view fills the rest.
pub fn view(
    root: &mut egui::Ui,
    draft: &mut SettingsDraft,
    local_ai: &mut LocalAi,
    setup: &mut setup::Panel,
    folders: &FolderIndex,
    config_path: &Path,
    app: AppState,
) -> Option<SettingsAction> {
    let ctx = &root.ctx().clone();
    let ai = app.ai;
    let t = t();
    let mut action = None;
    // Handle the keyboard before the fields: Esc in a text field only ends the input.
    let typing = ctx.memory(|m| m.focused().is_some());
    let (escape, save_key) =
        ctx.input(|i| (i.key_pressed(egui::Key::Escape), i.modifiers.command && i.key_pressed(egui::Key::S)));
    let modal_open = draft.leave.is_some() || draft.conflict;
    if !modal_open {
        if save_key {
            action = Some(SettingsAction::Save);
        } else if escape && !typing {
            action = draft.request_leave(Leave::Back);
        }
    }
    let dirty = draft.is_dirty();
    let problems = problems(draft);
    egui::Panel::bottom("settings-actions").frame(theme::bar_frame(&ctx.global_style())).show(root, |ui| {
        for p in &problems {
            ui.horizontal(|ui| {
                ui.label(theme::icon_text(theme::icon::WARN, 16.0, status_warn()));
                ui.add(egui::Label::new(RichText::new(&p.text).color(status_warn())).truncate());
                if draft.tab != p.tab || p.rule.is_some() {
                    let jump = theme::ghost_button(ui, t.show).on_hover_text(t.show_hint);
                    if jump.clicked() {
                        draft.tab = p.tab;
                        if p.rule.is_some() {
                            draft.editing_rule = p.rule;
                        }
                    }
                }
            });
        }
        ui.horizontal(|ui| {
            ui.menu_button(RichText::new(t.config_file).size(text::SMALL).color(muted()), |ui| {
                theme::list_style(ui);
                ui.label(RichText::new(config_path.display().to_string()).small().color(muted()));
                if ui.button(t.open_in_editor).clicked() {
                    open_path(config_path);
                    ui.close();
                }
                if ui.button(t.reload_discard).clicked() {
                    action = draft.request_leave(Leave::Reload);
                    ui.close();
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(theme::primary_button(t.save))
                    .on_hover_text(format!("{}+S", crate::i18n::general::t().key_ctrl))
                    .clicked()
                {
                    action = Some(SettingsAction::Save);
                }
                if ui.add(theme::secondary_button(t.cancel)).on_hover_text(t.cancel_hint).clicked() {
                    action = draft.request_leave(Leave::Back);
                }
                if let Some(e) = &draft.save_error {
                    ui.add(egui::Label::new(RichText::new((t.not_saved)(e)).color(status_err())).truncate());
                } else if dirty {
                    ui.label(RichText::new(t.unsaved).color(status_warn()));
                    theme::status_dot(ui, status_warn(), true);
                }
            });
        });
    });
    if let Some(a) = leave_dialog(ctx, draft) {
        action = Some(a);
    }
    if let Some(a) = conflict_dialog(ctx, draft) {
        action = Some(a);
    }
    // Ctrl+Tab / Ctrl+Shift+Tab switch the tab (§8) while no question is open. Check with
    // Shift first: otherwise Ctrl alone also matches Ctrl+Shift.
    if draft.leave.is_none() && !draft.conflict {
        let back = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Tab));
        let forward = !back && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Tab));
        if back || forward {
            draft.tab = draft.tab.step(if back { -1 } else { 1 });
        }
    }
    let setup_label = theme::with_icon(theme::icon::SPARKLES, Tab::Setup.label());
    let labels: Vec<&str> = Tab::MAIN.iter().map(|tab| tab.label()).chain([setup_label.as_str()]).collect();
    egui::Panel::left("settings-tabs")
        .resizable(false)
        .exact_size(tab_bar_width(ctx, &labels))
        .frame(theme::bar_frame(&ctx.global_style()))
        .show(root, |ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(t.title).size(text::VALUE).family(theme::semibold()));
            ui.add_space(12.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            for tab in Tab::MAIN {
                if theme::nav_item(ui, draft.tab == tab, tab.label(), text::LARGE).clicked() {
                    draft.tab = tab;
                }
            }
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            let resp = theme::nav_item(ui, draft.tab == Tab::Setup, &setup_label, text::LARGE);
            if resp.on_hover_text(t.setup_with_ai_hint).clicked() {
                draft.tab = Tab::Setup;
            }
        });
    egui::CentralPanel::default().frame(theme::canvas_frame(&ctx.global_style())).show(root, |ui| {
        if draft.tab == Tab::Archive {
            // Own scroll bar only for the folder rows: the archive folder and the column headers stay in place.
            let w = ui.available_width().min(CONTENT_MAX_WIDTH);
            ui.allocate_ui_with_layout(Vec2::new(w, ui.available_height()), Layout::top_down(Align::Min), |ui| {
                ui.set_width(w);
                folders_tab(ui, draft);
            });
            return;
        }
        ScrollArea::vertical().id_salt(("settings", draft.tab as u8)).auto_shrink([false, false]).show(ui, |ui| {
            // Fixed width: contents must not widen the view.
            let w = (ui.available_width() - 14.0).min(CONTENT_MAX_WIDTH);
            ui.set_width(w);
            ui.set_max_width(w);
            match draft.tab {
                Tab::Archive => {}
                Tab::Names => names_tab(ui, draft),
                Tab::Rules => rules_tab(ui, draft, folders),
                Tab::Ai => ai_tab(ui, draft, local_ai, ai),
                Tab::Scan => about::scan_tab(ui, &mut draft.config.scan),
                Tab::General => {
                    if let Some(a) = about::general_tab(ui, app.mode, app.updates) {
                        action = Some(SettingsAction::General(a));
                    }
                }
                Tab::Setup => match setup::panel_ui(ui, setup, &draft.build(), folders, ai, draft.config.llm.enabled) {
                    Some(setup::PanelAction::Generate) => action = Some(SettingsAction::GenerateSetup),
                    Some(setup::PanelAction::Apply) => {
                        if let Some(p) = setup.proposal.take() {
                            let mut cfg = draft.build();
                            let notes = setup::apply(&p, &mut cfg, folders, setup.take);
                            *draft = draft.rebuilt(&cfg);
                            setup.applied = Some(notes);
                        }
                    }
                    None => {}
                },
            }
        });
    });
    action
}

/// Something in the draft that would not be saved as intended – with the tab it is on.
struct Problem {
    tab: Tab,
    text: String,
    /// Affected special rule (to expand it).
    rule: Option<usize>,
}

/// Checks the draft: missing folders and entries that would be dropped when saving.
fn problems(draft: &SettingsDraft) -> Vec<Problem> {
    let t = t();
    let mut out = Vec::new();
    let c = &draft.config;
    let at = |tab, text: String| Problem { tab, text, rule: None };
    if c.archive_root.trim().is_empty() {
        out.push(at(Tab::Archive, t.no_archive_folder.into()));
    } else if !Path::new(c.archive_root.trim()).is_dir() {
        out.push(at(Tab::Archive, (t.archive_folder_missing)(c.archive_root.trim())));
    }
    if let Some(text) = draft.folders.fallback_problem(c) {
        out.push(at(Tab::Archive, text));
    }
    for (i, r) in draft.rules.iter().enumerate() {
        let (words, folder) = (!r.keywords.trim().is_empty(), !r.folder.trim().is_empty());
        if words != folder {
            let name = Some(r.name.trim()).filter(|n| !n.is_empty());
            out.push(Problem { tab: Tab::Rules, text: (t.rule_incomplete)(name, i + 1, words), rule: Some(i) });
        }
    }
    let broken = draft
        .folder_templates
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| l.split_once('=').is_none_or(|(k, v)| k.trim().is_empty() || v.trim().is_empty()))
        .count();
    if broken > 0 {
        out.push(at(Tab::Names, (t.broken_templates)(broken)));
    }
    out
}

/// Question on unsaved changes: save, discard or keep editing.
fn leave_dialog(ctx: &egui::Context, draft: &mut SettingsDraft) -> Option<SettingsAction> {
    let leave = draft.leave?;
    let t = t();
    let mut action = None;
    let what = match leave {
        Leave::Back => t.leave_back,
        Leave::Reload => t.reload,
        Leave::Quit => t.quit,
    };
    let enter = theme::dialog_enter(ctx);
    let modal = theme::dialog(egui::Id::unique("settings-leave")).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.label(theme::dialog_title(t.unsaved));
        ui.add_space(8.0);
        ui.label((t.leave_question)(what));
        ui.add_space(16.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(theme::primary_button(t.save)).clicked() || enter {
                action = Some(if leave == Leave::Quit { SettingsAction::SaveAndQuit } else { SettingsAction::Save });
            }
            let discard = match leave {
                Leave::Back => SettingsAction::Cancel,
                Leave::Reload => SettingsAction::Reload,
                Leave::Quit => SettingsAction::Quit,
            };
            if ui.add(theme::secondary_button(t.discard)).clicked() {
                action = Some(discard);
            }
            if ui.add(theme::secondary_button(t.keep_editing)).clicked() {
                draft.leave = None;
            }
        });
    });
    if action.is_some() || modal.should_close() {
        draft.leave = None;
    }
    action
}

/// The file was changed elsewhere since opening: overwrite or reload?
fn conflict_dialog(ctx: &egui::Context, draft: &mut SettingsDraft) -> Option<SettingsAction> {
    if !draft.conflict {
        return None;
    }
    let t = t();
    let mut action = None;
    let enter = theme::dialog_enter(ctx);
    let modal = theme::dialog(egui::Id::unique("settings-conflict")).show(ctx, |ui| {
        ui.set_width(460.0);
        ui.label(theme::dialog_title(t.conflict_title));
        ui.add_space(8.0);
        ui.label(t.conflict_text);
        ui.add_space(16.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(theme::primary_button(t.overwrite)).clicked() || enter {
                action = Some(SettingsAction::SaveAnyway);
            }
            if ui.add(theme::secondary_button(t.reload)).on_hover_text(t.reload_hint).clicked() {
                action = Some(SettingsAction::Reload);
            }
            if ui.add(theme::secondary_button(t.back)).clicked() {
                draft.conflict = false;
            }
        });
    });
    if action.is_some() || modal.should_close() {
        draft.conflict = false;
    }
    action
}

pub fn open_path(path: &Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

fn folders_tab(ui: &mut egui::Ui, draft: &mut SettingsDraft) {
    let SettingsDraft { config: c, folders: view, .. } = draft;
    let t = t();
    // While the path is being typed or the depth dragged, the overview does not rescan.
    let mut editing = false;
    theme::labeled(ui, t.archive_folder, |ui| {
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 150.0).clamp(200.0, 640.0);
            let resp = theme::field(ui, TextEdit::singleline(&mut c.archive_root).desired_width(w));
            editing |= resp.has_focus();
            if ui.add(theme::secondary_button(theme::with_icon(theme::icon::FOLDER_SEARCH, t.browse))).clicked()
                && let Some(p) = rfd::FileDialog::new().set_title(t.pick_archive_folder).pick_folder()
            {
                c.archive_root = p.display().to_string();
            }
        });
    });
    theme::labeled(ui, t.fallback_folder, |ui| {
        ui.horizontal(|ui| {
            view.fallback_ui(ui, c);
            hint(ui, t.fallback_hint);
        });
    });
    if !Path::new(&c.archive_root).is_dir() {
        ui.label(RichText::new(t.folder_missing).small().color(status_err()));
    }
    theme::collapsing(t.advanced).id_salt("ordner-erweitert").show(ui, |ui| {
        theme::labeled(ui, t.scan_depth, |ui| {
            let resp = ui.add(egui::DragValue::new(&mut c.scan_depth).range(1..=8)).on_hover_text(t.scan_depth_hint);
            editing |= resp.dragged() || resp.has_focus();
        });
        theme::checkbox(ui, &mut c.allow_new_folders, t.allow_new_folders);
        // What happens to the file when filing belongs to the archive, not to the file name.
        theme::checkbox(ui, &mut c.copy_instead_of_move, t.copy_instead_of_move).on_hover_text(t.copy_hint);
        theme::checkbox(ui, &mut c.write_tags, t.write_tags).on_hover_text(t.write_tags_hint);
        theme::checkbox(ui, &mut c.log_decisions, t.log_decisions).on_hover_text(t.log_decisions_hint);
    });

    theme::section(ui, t.folders);
    view.ui(ui, c, editing);
}

/// Date formats to choose from; their names are in the same order in `date_formats` of the texts.
const DATE_FORMATS: [&str; 3] = ["%y%m%d", "%Y-%m-%d", "%Y%m%d"];

fn names_tab(ui: &mut egui::Ui, draft: &mut SettingsDraft) {
    let c = &mut draft.config;
    let t = t();
    theme::section(ui, t.layout);
    hint(ui, t.layout_hint);
    ui.add_space(4.0);
    schema::blocks(ui, &mut c.filename_template);
    ui.add_space(8.0);
    ui.horizontal_top(|ui| {
        theme::labeled(ui, t.separator, |ui| schema::separator(ui, &mut c.filename_template));
        ui.add_space(16.0);
        theme::labeled(ui, t.part_date, |ui| {
            let shown = DATE_FORMATS
                .iter()
                .position(|f| *f == c.date_format)
                .map_or_else(|| (t.own_date_format)(&c.date_format), |i| t.date_formats[i].to_string());
            theme::combo("date-format").selected_text(shown).width(340.0).show_ui(ui, |ui| {
                for (f, label) in DATE_FORMATS.into_iter().zip(t.date_formats) {
                    ui.selectable_value(&mut c.date_format, f.to_string(), label);
                }
            });
        });
    });
    theme::labeled(ui, t.as_text, |ui| schema::text_row(ui, &mut c.filename_template));
    theme::checkbox(ui, &mut c.compact_filenames, t.compact);
    theme::checkbox(ui, &mut c.person_from_folder, t.person_from_folder).on_hover_text(t.person_from_folder_hint);
    let rules = NameRules { compact: c.compact_filenames, max_len: c.max_filename_length };
    let d = |y, m, day| chrono::NaiveDate::from_ymd_opt(y, m, day).map(DocDate::full);
    let examples = [
        TemplateFields {
            date: d(2026, 9, 25),
            doctype: "Steuerbescheid",
            sender: "Finanzamt",
            subject: "2025",
            ..Default::default()
        },
        TemplateFields {
            date: DocDate::year_month(2016, 1),
            doctype: "Gehaltsabrechnung",
            sender: "Acme",
            subject: "2016-01",
            person: "Max",
            ..Default::default()
        },
        TemplateFields {
            date: None,
            doctype: "Mitteilung",
            sender: "BZSt",
            subject: "Steuer-ID",
            ..Default::default()
        },
    ];
    ui.add_space(8.0);
    let examples_frame = egui::Frame::new().fill(tokens::surface_muted()).corner_radius(theme::radius::LG);
    examples_frame.inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        hint(ui, t.examples);
        for e in &examples {
            let name = naming::render_filename(&c.filename_template, &c.date_format, e, rules);
            ui.label(RichText::new(format!("{name}.pdf")).monospace());
        }
    });

    // "Lists", not "parts": the parts of the name above already go by that name.
    theme::section(ui, t.lists);
    list_section(ui, "doctypes", t.doctypes, &mut draft.doctypes, 10, t.doctypes_help);
    list_section(ui, "persons", t.persons, &mut draft.persons, 5, t.persons_help);
    list_section(ui, "senders", t.senders, &mut draft.senders, 8, t.senders_help);
    list_section(ui, "extras", t.extras, &mut draft.extras, 4, t.extras_help);

    theme::section(ui, t.instructions);
    hint(ui, t.instructions_help);
    theme::text_area(
        ui,
        TextEdit::multiline(&mut c.llm.instructions)
            .hint_text(t.instructions_placeholder)
            .desired_rows(6)
            .desired_width(f32::INFINITY),
    );

    ui.add_space(12.0);
    theme::collapsing(t.advanced).id_salt("namen-erweitert").show(ui, |ui| {
        ui.horizontal_top(|ui| {
            theme::labeled(ui, t.date_format, |ui| {
                let edit =
                    TextEdit::singleline(&mut c.date_format).desired_width(120.0).font(egui::TextStyle::Monospace);
                theme::field(ui, edit).on_hover_text(t.date_format_hint);
            });
            ui.add_space(16.0);
            theme::labeled(ui, t.max_length, |ui| {
                ui.add(egui::DragValue::new(&mut c.max_filename_length).range(30..=200));
            });
        });
        ui.add_space(8.0);
        theme::labeled(ui, t.folder_templates, |ui| {
            hint(ui, t.folder_templates_help);
            list_box(ui, "folder-templates", &mut draft.folder_templates, 3);
        });
        ui.add_space(8.0);
        theme::labeled(ui, t.keep_names, |ui| {
            hint(ui, t.keep_names_help);
            theme::field(ui, TextEdit::singleline(&mut draft.keep_names).desired_width(f32::INFINITY));
        });
    });
}

fn rules_tab(ui: &mut egui::Ui, draft: &mut SettingsDraft, folders: &FolderIndex) {
    let t = t();
    hint(ui, t.rules_intro);
    theme::section(ui, &(t.rules_count)(draft.rules.len()));
    rules_list(ui, &mut draft.rules, &mut draft.editing_rule, &folders.all);
    ui.add_space(12.0);
    rule_probe(ui, draft);
}

/// Tries the rules on a text (on opening: the text of the current document).
fn rule_probe(ui: &mut egui::Ui, draft: &mut SettingsDraft) {
    let t = t();
    theme::collapsing(t.probe).id_salt("rule-probe").default_open(false).show(ui, |ui| {
        hint(ui, t.probe_hint);
        theme::text_area(
            ui,
            TextEdit::multiline(&mut draft.rule_probe)
                .hint_text(t.probe_placeholder)
                .desired_width(f32::INFINITY)
                .desired_rows(4),
        );
        if draft.rule_probe.trim().is_empty() {
            return;
        }
        let parsed = rules_from_drafts(&draft.rules);
        match archive::match_rule_index(&parsed, &draft.rule_probe) {
            Some(hit) => {
                let rule = &parsed[hit];
                let words = archive::rule_hits(rule, &archive::normalize(&draft.rule_probe)).join(", ");
                let text = (t.probe_hit)(hit + 1, &rule.display_name(), &rule.folder, &words);
                ui.label(RichText::new(text).color(status_ok()));
            }
            None => {
                ui.label(RichText::new(t.probe_none).color(muted()));
            }
        }
    });
}

/// Rules as a compact list; the clicked rule expands its fields.
fn rules_list(ui: &mut egui::Ui, rules: &mut Vec<RuleDraft>, editing: &mut Option<usize>, folders: &[String]) {
    let t = t();
    let mut remove = None;
    let mut shift: Option<(usize, isize)> = None;
    let n = rules.len();
    for (i, rule) in rules.iter_mut().enumerate() {
        let open = *editing == Some(i);
        let fill = if open { tokens::surface_muted() } else { egui::Color32::TRANSPARENT };
        egui::Frame::new().fill(fill).corner_radius(theme::radius::MD).inner_margin(Margin::symmetric(8, 4)).show(
            ui,
            |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{:>2}.", i + 1)).color(muted()).monospace());
                    theme::checkbox(ui, &mut rule.enabled, "").on_hover_text(if rule.enabled {
                        t.rule_on
                    } else {
                        t.rule_off
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::ghost_button(ui, if open { t.done } else { t.edit }).clicked() {
                            *editing = if open { None } else { Some(i) };
                        }
                        if theme::arrow_button(ui, Arrow::Down, i + 1 < n, t.priority_down) {
                            shift = Some((i, 1));
                        }
                        if theme::arrow_button(ui, Arrow::Up, i > 0, t.priority_up) {
                            shift = Some((i, -1));
                        }
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            let title = if rule.name.trim().is_empty() {
                                rule.keywords.split(',').next().unwrap_or_default().trim().to_string()
                            } else {
                                rule.name.trim().to_string()
                            };
                            let color = if rule.enabled { tokens::fg() } else { muted() };
                            let resp = ui.add(egui::Label::new(RichText::new(title).color(color)).sense(Sense::CLICK));
                            if resp.clicked() {
                                *editing = if open { None } else { Some(i) };
                            }
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("→ {}", rule.folder)).size(text::SMALL).color(muted()),
                                )
                                .truncate(),
                            );
                        });
                    });
                });
                if open {
                    rule_fields(ui, rule, i, folders, &mut remove);
                }
            },
        );
    }
    if let Some(i) = remove {
        rules.remove(i);
        *editing = None;
    } else if let Some((i, delta)) = shift {
        let j = i as isize + delta;
        if j >= 0 && (j as usize) < rules.len() {
            rules.swap(i, j as usize);
            if *editing == Some(i) {
                *editing = Some(j as usize);
            }
        }
    }
    ui.add_space(4.0);
    if ui.add(theme::secondary_button(theme::with_icon(theme::icon::PLUS, t.add_rule))).clicked() {
        rules.push(RuleDraft::empty());
        *editing = Some(rules.len() - 1);
    }
}

fn rule_fields(ui: &mut egui::Ui, rule: &mut RuleDraft, i: usize, folders: &[String], remove: &mut Option<usize>) {
    ui.add_space(4.0);
    let t = t();
    // Two columns with the label above the field: on the left what the rule matches, on the right what it sets.
    let gap = 24.0;
    let col = ((ui.available_width() - gap) / 2.0).max(220.0);
    // Inner width of the fields: without their padding they would stick out of the column.
    let inner = col - 2.0 * theme::FIELD_MARGIN;
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(Vec2::new(col, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(col);
            theme::labeled(ui, t.rule_name, |ui| {
                theme::field(ui, TextEdit::singleline(&mut rule.name).hint_text(t.optional).desired_width(inner));
            });
            theme::labeled(ui, t.keywords, |ui| {
                let edit = TextEdit::singleline(&mut rule.keywords).hint_text(t.keywords_placeholder);
                theme::field(ui, edit.desired_width(inner)).on_hover_text(t.keywords_hint);
                theme::checkbox(ui, &mut rule.match_all, t.match_all);
            });
            theme::labeled(ui, t.require, |ui| {
                let edit = TextEdit::singleline(&mut rule.require).hint_text(t.require_placeholder);
                theme::field(ui, edit.desired_width(inner)).on_hover_text(t.require_hint);
            });
            theme::labeled(ui, t.exclude, |ui| {
                let edit = TextEdit::singleline(&mut rule.exclude).hint_text(t.exclude_placeholder);
                theme::field(ui, edit.desired_width(inner)).on_hover_text(t.exclude_hint);
            });
        });
        ui.add_space(gap - ui.spacing().item_spacing.x);
        ui.allocate_ui_with_layout(Vec2::new(col, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(col);
            theme::labeled(ui, t.target_folder, |ui| {
                ui.horizontal(|ui| {
                    let w = inner - 80.0 - ui.spacing().item_spacing.x;
                    let edit = TextEdit::singleline(&mut rule.folder).hint_text(t.target_folder_placeholder);
                    theme::field(ui, edit.desired_width(w));
                    let pick = theme::combo(("rule-folder", i)).selected_text(t.folder_pick).width(80.0).height(320.0);
                    pick.show_ui(ui, |ui| {
                        ui.set_min_width(320.0);
                        for folder in folders {
                            if ui.selectable_label(rule.folder == *folder, folder).clicked() {
                                rule.folder = folder.clone();
                            }
                        }
                    });
                });
            });
            theme::labeled(ui, t.sets, |ui| {
                let w = (col - 2.0 * ui.spacing().item_spacing.x) / 3.0 - 2.0 * theme::FIELD_MARGIN;
                ui.horizontal(|ui| {
                    theme::field(
                        ui,
                        TextEdit::singleline(&mut rule.doctype).hint_text(t.part_doctype).desired_width(w),
                    );
                    theme::field(ui, TextEdit::singleline(&mut rule.sender).hint_text(t.part_sender).desired_width(w));
                    theme::field(ui, TextEdit::singleline(&mut rule.person).hint_text(t.part_person).desired_width(w));
                });
            });
        });
    });
    ui.horizontal(|ui| {
        if theme::ghost_button(ui, &theme::with_icon(theme::icon::TRASH, t.delete_rule)).clicked() {
            *remove = Some(i);
        }
    });
}

fn ai_tab(ui: &mut egui::Ui, draft: &mut SettingsDraft, local_ai: &mut LocalAi, ai: &AiStatus) {
    let c = &mut draft.config;
    let t = t();
    let mut test = false;
    ui.horizontal(|ui| {
        theme::checkbox(ui, &mut c.llm.enabled, RichText::new(t.use_ai).size(text::LARGE));
        if c.llm.enabled {
            theme::status_dot(ui, if ai.ready { status_ok() } else { status_warn() }, ai.ready);
            ui.add(
                egui::Label::new(RichText::new(&ai.text).color(if ai.ready { status_ok() } else { status_warn() }))
                    .truncate(),
            );
        }
    });
    hint(ui, t.ai_intro);
    ui.add_enabled_ui(c.llm.enabled, |ui| {
        theme::section(ui, t.where_ai_runs);
        ui.horizontal(|ui| {
            theme::radio_value(ui, &mut c.llm.backend, LlmBackend::Embedded, t.embedded);
            theme::radio_value(ui, &mut c.llm.backend, LlmBackend::Server, t.server);
        });
        if c.llm.backend == LlmBackend::Embedded {
            theme::collapsing(t.model_and_gpu)
                .id_salt("llm-model")
                .default_open(!ai.ready)
                .show(ui, |ui| local_ai.settings_ui(ui, &mut c.llm));
        } else {
            ui.horizontal_top(|ui| {
                theme::labeled(ui, t.server_url, |ui| {
                    theme::field(ui, TextEdit::singleline(&mut c.llm.base_url).desired_width(320.0));
                });
                ui.add_space(16.0);
                theme::labeled(ui, t.model, |ui| {
                    ui.horizontal(|ui| {
                        let edit = TextEdit::singleline(&mut c.llm.model).hint_text(t.model_placeholder);
                        theme::field(ui, edit.desired_width(240.0));
                        if let Some(Ok(models)) = &draft.test_result {
                            theme::combo("model-pick").selected_text(t.model_list).width(70.0).show_ui(ui, |ui| {
                                for m in models {
                                    if ui.selectable_label(c.llm.model == *m, m).clicked() {
                                        c.llm.model = m.clone();
                                    }
                                }
                            });
                        }
                    });
                });
            });
            ui.horizontal_top(|ui| {
                theme::labeled(ui, t.api_key, |ui| {
                    let edit = TextEdit::singleline(&mut c.llm.api_key).password(true).hint_text(t.api_key_placeholder);
                    theme::field(ui, edit.desired_width(320.0));
                });
                ui.add_space(16.0);
                theme::labeled(ui, t.timeout, |ui| {
                    ui.add(egui::DragValue::new(&mut c.llm.timeout_secs).range(5..=900));
                });
            });
            if let Some(rx) = &draft.test_rx
                && let Ok(result) = rx.try_recv()
            {
                draft.test_result = Some(result);
                draft.test_rx = None;
            }
            ui.horizontal(|ui| {
                let testing = draft.test_rx.is_some();
                if ui
                    .add_enabled(!testing, theme::secondary_button(t.test_connection))
                    .on_disabled_hover_text(t.test_running)
                    .clicked()
                {
                    test = true;
                }
                if testing {
                    ui.spinner();
                    ui.label(RichText::new(t.connecting).color(muted()));
                }
                match &draft.test_result {
                    _ if testing => {}
                    Some(Ok(models)) => {
                        ui.label(RichText::new((t.connected)(models.len())).color(status_ok()));
                    }
                    Some(Err(e)) => {
                        ui.label(RichText::new(e).color(status_err()));
                    }
                    None => {}
                }
            });
            hint(ui, t.server_recommendation);
        }

        theme::section(ui, t.working);
        theme::checkbox(ui, &mut c.llm.folder_steps, t.folder_steps).on_hover_text(t.folder_steps_hint);
        theme::checkbox(ui, &mut c.llm.use_vision, t.use_vision);
    })
    .response
    .on_disabled_hover_text(t.ai_off);

    ui.add_space(12.0);
    theme::collapsing(t.ocr_section).id_salt("ocr").show(ui, |ui| {
        hint(ui, t.ocr_hint);
        // Loads Tesseract the first time (on Windows: unpacks the DLL); after that the answer is fixed.
        theme::labeled(ui, t.ocr_engine, |ui| match crate::tesseract::version() {
            Ok(v) => ui.label(RichText::new(format!("Tesseract {v}")).color(status_ok())),
            Err(e) => ui.add(egui::Label::new(RichText::new(format!("{e:#}")).color(status_err())).wrap()),
        });
        ocr_languages(ui, &mut c.ocr);
        ui.horizontal_top(|ui| {
            theme::labeled(ui, t.ocr_quality, |ui| {
                theme::segments(ui, theme::CONTROL, |ui| {
                    for (q, label) in [(OcrQuality::Fast, t.ocr_fast), (OcrQuality::Best, t.ocr_best)] {
                        theme::segment_value(ui, &mut c.ocr.quality, q, label).on_hover_text(t.ocr_quality_hint);
                    }
                });
            });
            ui.add_space(16.0);
            theme::labeled(ui, t.max_pages, |ui| {
                ui.add(egui::DragValue::new(&mut c.max_pages).range(1..=50)).on_hover_text(t.max_pages_hint);
            });
        });
    });
    if test {
        let (tx, rx) = std::sync::mpsc::channel();
        let (llm, ctx) = (draft.config.llm.clone(), ui.ctx().clone());
        let started = std::thread::Builder::new().name("connection-test".into()).spawn(move || {
            let _ = tx.send(LlmClient::new(&llm).list_models().map_err(|e| format!("{e:#}")));
            ctx.request_repaint();
        });
        match started {
            Ok(_) => draft.test_rx = Some(rx),
            Err(e) => draft.test_result = Some(Err(e.to_string())),
        }
    }
}

/// Tesseract codes of the UI languages; they are offered as chips, all others as text.
const OCR_LANGUAGES: [(Lang, &str); 5] =
    [(Lang::De, "deu"), (Lang::En, "eng"), (Lang::Fr, "fra"), (Lang::Es, "spa"), (Lang::It, "ita")];

/// Document languages: filter chips for the five UI languages, further Tesseract codes as
/// text, and a hint.
fn ocr_languages(ui: &mut egui::Ui, ocr: &mut OcrConfig) {
    let t = t();
    let known = |code: &str| OCR_LANGUAGES.iter().any(|(_, k)| *k == code);
    let mut list = ocr.language_list();
    let mut changed = false;
    ui.label(theme::field_label(t.ocr_languages));
    ui.horizontal_wrapped(|ui| {
        for (lang, code) in OCR_LANGUAGES {
            let on = list.iter().any(|c| c == code);
            if theme::filter_chip(ui, on, lang.name()).on_hover_text(code).clicked() {
                if on {
                    list.retain(|c| c != code);
                } else {
                    list.push(code.to_string());
                }
                changed = true;
            }
        }
    });

    // Own buffer while the field has focus: a started "nld+" stays while typing.
    let id = ui.scope_id().with("ocr-more");
    let extras = list.iter().filter(|c| !known(c)).cloned().collect::<Vec<_>>().join("+");
    let mut buffer = ui.data(|d| d.get_temp::<String>(id)).unwrap_or(extras);
    ui.horizontal(|ui| {
        ui.label(RichText::new(t.ocr_more).color(muted()));
        let resp = theme::field(ui, TextEdit::singleline(&mut buffer).hint_text("nld+pol").desired_width(160.0))
            .on_hover_text(t.ocr_more_hint);
        if resp.changed() {
            list.retain(|c| known(c));
            let typed = OcrConfig { languages: buffer.to_lowercase(), ..OcrConfig::default() }.language_list();
            for code in typed {
                if !list.contains(&code) {
                    list.push(code);
                }
            }
            changed = true;
        }
        if resp.has_focus() {
            ui.data_mut(|d| d.insert_temp(id, buffer.clone()));
        } else {
            ui.data_mut(|d| d.remove::<String>(id));
        }
    });
    ui.label(RichText::new(t.ocr_languages_hint).small().color(muted()));
    if changed {
        ocr.set_languages(&list);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §10 of the design system: the tab names fit into the bar in every language with 5 % margin. Measured
    /// in semibold (the selected tab) with the fonts `theme::apply` loads – Segoe UI on Windows, Noto or
    /// DejaVu on Linux. For this, the bar grows to at most 240 px so the content keeps its room.
    #[test]
    fn tab_labels_fit_the_bar_in_every_language() {
        use crate::i18n::settings::{DE, EN, ES, FR, IT};
        let ctx = egui::Context::default();
        theme::apply(&ctx, theme::Mode::Light);
        let _ = ctx.run_ui(Default::default(), |_| {});
        let font = egui::FontId::new(text::LARGE, theme::semibold());
        for (lang, t) in [("de", &DE), ("en", &EN), ("fr", &FR), ("es", &ES), ("it", &IT)] {
            let setup = theme::with_icon(theme::icon::SPARKLES, t.setup_with_ai);
            let labels = [t.tab_general, t.tab_scan, t.tab_archive, t.tab_names, t.tab_rules, t.tab_ai, &setup];
            let bar = tab_bar_width(&ctx, &labels);
            assert!(bar <= 240.0, "{lang}: tab bar {bar:.0} px wide, at most 240 px");
            for label in labels {
                let width =
                    ctx.fonts_mut(|f| f.layout_no_wrap(label.to_string(), font.clone(), egui::Color32::WHITE).size().x);
                let room = bar - TAB_BAR_PADDING;
                assert!(width * 1.05 <= room, "{lang}: \"{label}\" needs {width:.0} px, room is {room:.0} px");
            }
        }
    }

    /// §10: lines of the shortcut list (*General › Shortcuts*) have at most 40 characters.
    #[test]
    fn key_help_lines_are_short_in_every_language() {
        use crate::i18n::general::{DE, EN, ES, FR, IT};
        for (lang, t) in [("de", &DE), ("en", &EN), ("fr", &FR), ("es", &ES), ("it", &IT)] {
            let lines = [
                t.do_file,
                t.do_later,
                t.do_undo,
                t.do_pages,
                t.do_documents,
                t.do_zoom,
                t.do_save,
                t.do_tabs,
                t.do_back,
            ];
            for line in lines {
                assert!(line.chars().count() <= 40, "{lang}: \"{line}\" has more than 40 characters");
            }
        }
    }

    #[test]
    fn ctrl_tab_walks_the_tabs_in_a_circle() {
        assert!(Tab::General.step(1) == Tab::Scan);
        assert!(Tab::Ai.step(1) == Tab::Setup);
        assert!(Tab::Setup.step(1) == Tab::General, "after the last one comes the first again");
        assert!(Tab::General.step(-1) == Tab::Setup, "Ctrl+Shift+Tab goes from the first to the last");
    }

    #[test]
    fn list_fields_roundtrip() {
        let mut cfg = Config {
            persons: vec![
                Alias { name: "Max".into(), aliases: vec!["Maximilian".into(), "MM".into()] },
                Alias { name: "Lena".into(), aliases: vec![] },
            ],
            ..Default::default()
        };
        cfg.folder_notes.insert("02_Haus/_Strom".into(), "Strom, Einspeisung, PV".into());
        cfg.folder_templates.push(FolderTemplate {
            folder: "06_Haushalt/Anleitungen_Handbücher".into(),
            template: "{sender}_{subject}_Anleitung".into(),
        });
        cfg.rules.push(Rule {
            keywords: vec!["Grundsteuer".into()],
            exclude_keywords: vec!["Kontoauszug".into()],
            require_keywords: vec!["Musterstadt".into()],
            folder: "08_Steuern/Grundsteuer".into(),
            person: Some("Max".into()),
            ..Default::default()
        });
        let mut fresh = SettingsDraft::new(&cfg, Tab::Archive);
        assert!(!fresh.is_dirty(), "nothing is changed right after opening");
        assert!(matches!(fresh.request_leave(Leave::Back), Some(SettingsAction::Cancel)));
        fresh.doctypes.push_str("\nGedicht");
        assert!(fresh.is_dirty());
        assert!(fresh.request_leave(Leave::Quit).is_none() && fresh.leave == Some(Leave::Quit), "ask first");
        let cfg_now = fresh.build();
        let applied = fresh.rebuilt(&cfg_now);
        assert!(applied.is_dirty(), "applied but not saved yet");
        let mut draft = SettingsDraft::new(&cfg, Tab::Archive);
        assert_eq!(draft.persons, "Max: Maximilian, MM\nLena");
        assert_eq!(draft.build(), cfg);
        // Empty descriptions are dropped when saving.
        draft.folders.notes.push(("06_Haushalt".into(), "  ".into()));
        assert_eq!(draft.build().folder_notes.len(), 1);
    }

    #[test]
    fn problems_name_what_would_be_lost() {
        let dir = std::env::temp_dir();
        let cfg = Config { archive_root: dir.display().to_string(), ..Default::default() };
        let mut draft = SettingsDraft::new(&cfg, Tab::Archive);
        assert!(problems(&draft).is_empty());
        draft.rules.push(RuleDraft { keywords: "Grundsteuer".into(), ..RuleDraft::empty() });
        draft.rules.push(RuleDraft::empty());
        draft.folder_templates = "Haushalt/Anleitungen = {sender}_{subject}
Haushalt/Garten
"
        .into();
        let found = problems(&draft);
        assert_eq!(found.len(), 2, "the empty new rule does no harm");
        assert!(found[0].tab == Tab::Rules && found[0].rule == Some(0) && found[0].text.contains("keinen Zielordner"));
        assert!(found[1].tab == Tab::Names && found[1].text.starts_with("1 Zeile"));
        draft.config.archive_root = dir.join("gibt-es-nicht-scrinium").display().to_string();
        assert!(problems(&draft).iter().any(|p| p.tab == Tab::Archive));
    }

    #[test]
    fn presets_cover_defaults() {
        let cfg = Config::default();
        assert!(schema::PRESETS.contains(&cfg.filename_template.as_str()));
        assert!(DATE_FORMATS.contains(&cfg.date_format.as_str()));
    }
}
