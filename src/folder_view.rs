//! *Archive* settings tab: all folders of the archive as an expandable overview. Per folder a
//! checkbox (used or not), the description the AI uses to pick the folder, and keywords if needed.
//! Year folders and decade bundles are not listed one by one; Scrinium picks them itself.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use eframe::egui::{self, Align, Layout, Margin, RichText, ScrollArea, Stroke, TextEdit};

use crate::archive::{self, FolderWords};
use crate::config::Config;
use crate::i18n::folders;
use crate::theme::{self, text, tokens};
use tokens::{muted, status_warn};

const INDENT: f32 = 16.0;
const ROW_HEIGHT: f32 = theme::CONTROL;

/// A folder of the overview.
#[derive(Debug, Clone)]
struct Node {
    path: String,
    name: String,
    depth: usize,
    children: Vec<usize>,
    /// Year folders and decade bundles directly below.
    years: usize,
    /// Series of year folders below, e.g. `{year}_Max`, `{year}_Erika`.
    series: Vec<String>,
}

impl Node {
    /// Series the AI chooses between – only from two series on; the year logic picks a single one.
    fn choosable_series(&self) -> &[String] {
        if self.series.len() > 1 { &self.series } else { &[] }
    }
}

/// Folders read from the archive, including unused ones (so they can be switched on again).
#[derive(Debug, Clone, Default)]
struct Tree {
    root: String,
    depth: usize,
    nodes: Vec<Node>,
    roots: Vec<usize>,
    /// All paths read (comparison keys), year folders included.
    known: HashSet<String>,
}

impl Tree {
    fn scan(root: &str, depth: usize) -> Self {
        let paths = archive::scan_folders(Path::new(root), depth, &[]);
        Self::from_paths(root, depth, &paths)
    }

    fn from_paths(root: &str, depth: usize, paths: &[String]) -> Self {
        let mut tree = Tree { root: root.to_string(), depth, ..Default::default() };
        let mut index: BTreeMap<String, usize> = BTreeMap::new();
        for path in paths {
            tree.known.insert(key(path));
            let parts = archive::split_folder(path);
            let Some((last, parents)) = parts.split_last() else { continue };
            // Everything below a year folder or bundle is left out.
            if parents.iter().any(|p| is_period(p)) {
                continue;
            }
            let parent = match parents.is_empty() {
                true => None,
                false => match index.get(&key(&parents.join("/"))) {
                    Some(&p) => Some(p),
                    None => continue,
                },
            };
            if is_period(last) {
                if let Some(p) = parent {
                    let node = &mut tree.nodes[p];
                    node.years += 1;
                    if let Some((a, b, _)) = archive::year_span(last) {
                        let pattern = format!("{}{{year}}{}", &last[..a], &last[b..]);
                        if !node.series.iter().any(|s| key(s) == key(&pattern)) {
                            node.series.push(pattern);
                        }
                    }
                }
                continue;
            }
            let id = tree.nodes.len();
            tree.nodes.push(Node {
                path: parts.join("/"),
                name: last.to_string(),
                depth: parents.len(),
                children: Vec::new(),
                years: 0,
                series: Vec::new(),
            });
            index.insert(key(path), id);
            match parent {
                Some(p) => tree.nodes[p].children.push(id),
                None => tree.roots.push(id),
            }
        }
        tree
    }

    fn find(&self, path: &str) -> Option<&Node> {
        let k = key(path);
        self.nodes.iter().find(|n| key(&n.path) == k)
    }

    /// Does an entry for a year series (`Gehalt/{year}_Max`) appear as its own row in the overview?
    fn has_series(&self, entry: &str) -> bool {
        let parts = archive::split_folder(entry);
        let Some((last, parents)) = parts.split_last() else { return false };
        self.find(&parents.join("/")).is_some_and(|n| n.choosable_series().iter().any(|s| key(s) == key(last)))
    }
}

fn is_period(seg: &str) -> bool {
    archive::year_span(seg).is_some() || archive::is_decade_bundle(seg)
}

/// Comparison key of a folder path (ignoring case, umlauts and separators).
fn key(path: &str) -> String {
    archive::normalize(&archive::split_folder(path).join("/"))
}

/// Editing state of the folder overview.
#[derive(Debug, Default)]
pub struct FolderView {
    /// Unused folders (including their subfolders).
    pub exclude: Vec<String>,
    /// Former collection folders; on the first scan their subfolders become "unused".
    pub collapse: Vec<String>,
    /// Folder descriptions (folder, what belongs in it).
    pub notes: Vec<(String, String)>,
    /// Keywords and counter-words of the folders (*Recognised by* column).
    pub words: Vec<WordsDraft>,
    /// Folder clicked or edited last (comparison key); its row is highlighted.
    selected: Option<String>,
    filter: String,
    expanded: HashSet<String>,
    tree: Tree,
    /// Scan running in the background: archive folder, depth and the result.
    scan: Option<(String, usize, Receiver<Tree>)>,
    /// Width of the name column and the *Recognised by* column, matching the window width.
    name_w: f32,
    words_w: f32,
    /// Collection folders taken over, for a hint.
    migrated: Vec<String>,
}

impl FolderView {
    pub fn new(cfg: &Config) -> Self {
        Self {
            exclude: cfg.exclude.clone(),
            collapse: cfg.collapse.clone(),
            notes: cfg.folder_notes.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            words: cfg.folder_words.iter().map(|(k, v)| WordsDraft::from_config(k, v)).collect(),
            ..Default::default()
        }
    }

    /// Transfers unused folders, descriptions and keywords into the configuration.
    pub fn apply(&self, cfg: &mut Config) {
        let clean = |v: &[String]| v.iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let pairs = |v: &[(String, String)]| {
            v.iter()
                .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                .filter(|(k, v)| !k.is_empty() && !v.is_empty())
                .collect()
        };
        cfg.exclude = clean(&self.exclude);
        cfg.collapse = clean(&self.collapse);
        cfg.folder_notes = pairs(&self.notes);
        cfg.folder_words = self
            .words
            .iter()
            .map(|w| (w.path.trim().to_string(), w.to_config()))
            .filter(|(k, v)| !k.is_empty() && !v.is_empty())
            .collect();
    }

    /// Takes over the view and the scanned folders of an earlier draft (e.g. after *Apply*).
    pub fn keep_view(&mut self, old: &mut FolderView) {
        self.selected = old.selected.take();
        self.filter = std::mem::take(&mut old.filter);
        self.expanded = std::mem::take(&mut old.expanded);
        self.tree = std::mem::take(&mut old.tree);
        self.scan = old.scan.take();
        self.migrated = std::mem::take(&mut old.migrated);
    }

    fn listed(list: &[String], path: &str) -> bool {
        let k = key(path);
        list.iter().any(|e| key(e) == k)
    }

    fn set_listed(list: &mut Vec<String>, path: &str, on: bool) {
        let k = key(path);
        list.retain(|e| key(e) != k);
        if on {
            list.push(path.to_string());
        }
    }

    /// Is a parent folder unused?
    fn ancestor_in(list: &[String], path: &str) -> bool {
        let parts = archive::split_folder(path);
        (1..parts.len()).any(|n| Self::listed(list, &parts[..n].join("/")))
    }

    fn note_index(&self, path: &str) -> Option<usize> {
        pair_index(&self.notes, path)
    }

    fn described(&self) -> usize {
        filled(&self.notes)
    }

    /// Keywords and counter-words of a folder, if there are any.
    fn words_of(&self, path: &str) -> Option<&WordsDraft> {
        let k = key(path);
        self.words.iter().find(|w| key(&w.path) == k).filter(|w| !w.is_empty())
    }

    fn is_selected(&self, path: &str) -> bool {
        self.selected.as_deref() == Some(key(path).as_str())
    }

    /// Scans the archive in the background when the archive folder or the depth has changed. While
    /// `editing` (someone types the path or drags the depth), no new scan starts.
    fn refresh(&mut self, ctx: &egui::Context, cfg: &Config, editing: bool) {
        if let Some((root, depth, rx)) = &self.scan {
            match rx.try_recv() {
                Ok(tree) => {
                    self.scan = None;
                    self.tree = tree;
                    self.take_over_collections();
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    // Scan aborted: do not retry in a loop.
                    self.tree = Tree { root: root.clone(), depth: *depth, ..Default::default() };
                    self.scan = None;
                }
            }
        }
        let root = cfg.archive_root.trim();
        if editing || (self.tree.root == root && self.tree.depth == cfg.scan_depth) || !Path::new(root).is_dir() {
            return;
        }
        let (tx, rx) = channel();
        let (r, depth, ctx) = (root.to_string(), cfg.scan_depth, ctx.clone());
        let started = std::thread::Builder::new().name("folder-scan".into()).spawn(move || {
            let _ = tx.send(Tree::scan(&r, depth));
            ctx.request_repaint();
        });
        if started.is_ok() {
            self.scan = Some((root.to_string(), cfg.scan_depth, rx));
        }
    }

    /// Are the folders being scanned right now?
    pub fn scanning(&self) -> bool {
        self.scan.is_some()
    }

    /// Folders that qualify as fallback folder: used and without an unused parent folder.
    fn usable(&self) -> Vec<&str> {
        self.tree
            .nodes
            .iter()
            .filter(|n| !Self::listed(&self.exclude, &n.path) && !Self::ancestor_in(&self.exclude, &n.path))
            .map(|n| n.path.as_str())
            .collect()
    }

    /// Choice of the fallback folder: the top level of the archive or a used folder.
    pub fn fallback_ui(&self, ui: &mut egui::Ui, cfg: &mut Config) {
        let t = folders::t();
        let top = t.fallback_top;
        let current = cfg.fallback_folder.trim().to_string();
        let shown = if current.is_empty() { top.to_string() } else { current.clone() };
        let problem = self.fallback_problem(cfg).is_some();
        let text = RichText::new(shown).color(if problem { status_warn() } else { tokens::fg() });
        theme::combo("fallback-folder")
            .selected_text(text)
            .width(ui.available_width().min(420.0))
            .height(360.0)
            .show_ui(ui, |ui| {
                ui.set_min_width(380.0);
                if ui.selectable_label(current.is_empty(), top).clicked() {
                    cfg.fallback_folder.clear();
                }
                for path in self.usable() {
                    if ui.selectable_label(key(path) == key(&current), path).clicked() {
                        cfg.fallback_folder = path.to_string();
                    }
                }
            })
            .response
            .on_hover_text(t.fallback_hint);
    }

    /// Is something wrong with the fallback folder? (Only once the folders are scanned.)
    pub fn fallback_problem(&self, cfg: &Config) -> Option<String> {
        let t = folders::t();
        let f = cfg.fallback_folder.trim();
        if f.is_empty() || self.tree.nodes.is_empty() {
            return None;
        }
        if !self.tree.known.contains(&key(f)) {
            return Some((t.fallback_missing)(f));
        }
        if Self::listed(&self.exclude, f) || Self::ancestor_in(&self.exclude, f) {
            return Some((t.fallback_unused)(f));
        }
        None
    }

    /// Former collection folders: their subfolders (except year folders) count as unused. Collection folders
    /// that do not exist stay listed.
    fn take_over_collections(&mut self) {
        let mut kept = Vec::new();
        for c in std::mem::take(&mut self.collapse) {
            match self.tree.find(&c).map(|n| n.children.clone()) {
                Some(children) => {
                    for id in children {
                        let child = self.tree.nodes[id].path.clone();
                        Self::set_listed(&mut self.exclude, &child, true);
                    }
                    self.migrated.push(c);
                }
                None => kept.push(c),
            }
        }
        self.collapse = kept;
    }

    /// The overview with a fixed column header; the rows have their own scroll bar and fill the rest.
    /// `editing`: the archive folder or depth is being edited – rescan only afterwards.
    pub fn ui(&mut self, ui: &mut egui::Ui, cfg: &Config, editing: bool) {
        self.refresh(ui.ctx(), cfg, editing);
        self.name_w = (ui.available_width() * 0.28).clamp(220.0, 360.0);
        self.words_w = (ui.available_width() * 0.28).clamp(200.0, 420.0);
        let t = folders::t();
        let total = self.tree.nodes.len();
        ui.label(RichText::new(t.intro).small().color(muted()));
        if !self.migrated.is_empty() {
            ui.label(RichText::new((t.migrated)(&self.migrated.join(", "))).small().color(status_warn()));
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            theme::field(
                ui,
                TextEdit::singleline(&mut self.filter)
                    .hint_text(theme::with_icon(theme::icon::SEARCH, t.search_hint))
                    .desired_width(340.0),
            );
            if theme::ghost_button(ui, t.expand_all).clicked() {
                self.expanded =
                    self.tree.nodes.iter().filter(|n| !n.children.is_empty()).map(|n| key(&n.path)).collect();
            }
            if theme::ghost_button(ui, t.collapse_all).clicked() {
                self.expanded.clear();
            }
            if self.scanning() {
                ui.spinner();
                ui.label(RichText::new(t.scanning).small().color(muted()));
            } else {
                let mut status = (t.count_folders)(total, self.described());
                let with_words = self.words.iter().filter(|w| !w.is_empty()).count();
                if with_words > 0 {
                    status.push_str(", ");
                    status.push_str(&(t.count_with_words)(with_words));
                }
                ui.label(RichText::new(status).small().color(muted()));
            }
        });
        ui.label(RichText::new(t.legend).small().color(muted())).on_hover_text(t.words_help);
        ui.add_space(8.0);

        let rows = self.visible_rows();
        let flat = !self.filter.trim().is_empty();
        let spacing = ui.spacing().item_spacing.y;
        ui.spacing_mut().item_spacing.y = 0.0;
        header(ui, self.name_w, self.words_w);
        ScrollArea::vertical().id_salt("folder-rows").auto_shrink([false, false]).show(ui, |ui| {
            if rows.is_empty() {
                let msg = if self.scanning() {
                    ""
                } else if total == 0 {
                    t.no_folders
                } else {
                    t.no_match
                };
                ui.label(RichText::new(msg).color(muted()));
            }
            for id in rows {
                let node = self.tree.nodes[id].clone();
                let selected = self.is_selected(&node.path);
                table_row(ui, selected, |ui| self.row(ui, &node, flat));
            }
            ui.spacing_mut().item_spacing.y = spacing;
            self.leftovers(ui);
        });
        ui.spacing_mut().item_spacing.y = spacing;
    }

    /// Visible rows: expanded parts of the tree or, during a search, all hits.
    fn visible_rows(&self) -> Vec<usize> {
        let filter = archive::normalize(self.filter.trim());
        if !filter.is_empty() {
            return (0..self.tree.nodes.len())
                .filter(|&i| {
                    let n = &self.tree.nodes[i];
                    let note =
                        self.note_index(&n.path).map(|j| archive::normalize(&self.notes[j].1)).unwrap_or_default();
                    let words = self.words_of(&n.path).map(|w| archive::normalize(&w.to_config())).unwrap_or_default();
                    archive::normalize(&n.path).contains(&filter) || note.contains(&filter) || words.contains(&filter)
                })
                .collect();
        }
        let mut out = Vec::new();
        let mut stack: Vec<usize> = self.tree.roots.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            out.push(id);
            let node = &self.tree.nodes[id];
            if self.expanded.contains(&key(&node.path)) {
                stack.extend(node.children.iter().rev());
            }
        }
        out
    }

    fn row(&mut self, ui: &mut egui::Ui, node: &Node, flat: bool) {
        let t = folders::t();
        let path = node.path.as_str();
        let hidden_above = Self::ancestor_in(&self.exclude, path);
        let mut used = !Self::listed(&self.exclude, path);
        let active = used && !hidden_above;
        let (name_w, words_w) = (self.name_w, self.words_w);
        let mut focused = false;
        ui.horizontal(|ui| {
            name_column(ui, name_w, |ui| {
                ui.add_space(if flat { 0.0 } else { node.depth as f32 * INDENT });
                let k = key(path);
                let open = self.expanded.contains(&k);
                let expandable = !flat && !node.children.is_empty();
                let mut toggle = false;
                if expandable {
                    // Chevron (Lucide): right when closed, down when open; the click area is larger than the glyph.
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(18.0, ROW_HEIGHT), egui::Sense::CLICK);
                    let glyph = if open { theme::icon::CHEVRON_DOWN } else { theme::icon::CHEVRON_RIGHT };
                    let color = if resp.hovered() { tokens::accent_strong() } else { muted() };
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        glyph,
                        egui::FontId::proportional(theme::ICON_INLINE),
                        color,
                    );
                    let hover = if open { t.collapse_subfolders } else { t.show_subfolders };
                    toggle |= resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(hover).clicked();
                } else {
                    ui.add_space(18.0 + ui.spacing().item_spacing.x);
                }
                let hover = if hidden_above { t.parent_unused } else { t.use_hint };
                let check = ui.add_enabled(!hidden_above, theme::check(&mut used, ""));
                if check.on_hover_text(hover).on_disabled_hover_text(hover).changed() {
                    Self::set_listed(&mut self.exclude, path, !used);
                }
                let shown = if flat { path } else { node.name.as_str() };
                let mut label = RichText::new(shown).color(if active { tokens::fg() } else { muted() });
                if !active {
                    label = label.strikethrough();
                }
                let mut tip = path.to_string();
                if node.years > 0 {
                    tip.push('\n');
                    tip.push_str(t.years_tip);
                }
                // Expand only via the arrow; a click on the name highlights the row.
                let name = egui::Label::new(label).truncate().sense(egui::Sense::CLICK);
                if ui.add(name).on_hover_text(tip).clicked() {
                    self.selected = Some(k.clone());
                }
                if expandable {
                    ui.label(RichText::new(format!("({})", node.children.len())).size(text::SMALL).color(muted()))
                        .on_hover_text(t.subfolders);
                }
                if node.years > 0 {
                    ui.label(RichText::new(t.years_badge).size(text::SMALL).color(muted()));
                }
                if toggle {
                    if open {
                        self.expanded.remove(&k);
                    } else {
                        self.expanded.insert(k);
                    }
                }
            });
            let gap = ui.spacing().item_spacing.x;
            let mut scratch = String::new();
            let index = self.note_index(path);
            let note = match index {
                Some(i) => &mut self.notes[i].1,
                None => &mut scratch,
            };
            let edit = TextEdit::singleline(note)
                .hint_text(if active { t.note_hint } else { "" })
                .desired_width((ui.available_width() - words_w - gap - 2.0 * theme::FIELD_MARGIN).max(120.0));
            let unused = if hidden_above { t.parent_unused } else { t.folder_unused };
            let resp = ui.add_enabled_ui(active, |ui| theme::field(ui, edit)).inner.on_disabled_hover_text(unused);
            if resp.changed() && index.is_none() && !scratch.is_empty() {
                self.notes.push((path.to_string(), scratch));
            }
            focused |= resp.has_focus();
            ui.add_enabled_ui(active, |ui| {
                focused |= words_field(ui, &mut self.words, path, words_w);
            })
            .response
            .on_disabled_hover_text(unused);
        });
        // Several year series (`2026_Max`, `2026_Erika`): own features per series, e.g. the employer.
        if active {
            let indent = if flat { 0.0 } else { node.depth as f32 * INDENT };
            for pattern in node.choosable_series() {
                let entry = format!("{path}/{pattern}");
                ui.horizontal(|ui| {
                    name_column(ui, name_w, |ui| {
                        ui.add_space(indent + INDENT + 18.0 + 2.0 * ui.spacing().item_spacing.x);
                        ui.label(RichText::new(pattern.as_str()).size(text::SMALL).color(muted()));
                    });
                    let gap = ui.spacing().item_spacing.x;
                    let desc_w = (ui.available_width() - words_w - gap).max(120.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(desc_w, ROW_HEIGHT),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.set_width(desc_w);
                            ui.label(RichText::new(t.series_hint).size(text::SMALL).color(muted()));
                        },
                    );
                    focused |= words_field(ui, &mut self.words, &entry, words_w);
                });
            }
        }
        if focused {
            self.selected = Some(key(path));
        }
    }

    /// Entries without a matching folder (any more): descriptions and unused folders.
    fn leftovers(&mut self, ui: &mut egui::Ui) {
        if self.tree.nodes.is_empty() {
            return;
        }
        let missing = |p: &str| !self.tree.known.contains(&key(p));
        let orphan_notes: Vec<usize> = (0..self.notes.len())
            .filter(|&i| missing(&self.notes[i].0) && !self.notes[i].1.trim().is_empty())
            .collect();
        let orphan_lists: Vec<String> =
            self.exclude.iter().chain(&self.collapse).filter(|p| missing(p)).cloned().collect();
        let orphan_words: Vec<String> = self
            .words
            .iter()
            .filter(|w| !w.is_empty() && missing(&w.path) && !self.tree.has_series(&w.path))
            .map(|w| w.path.clone())
            .collect();
        if orphan_notes.is_empty() && orphan_lists.is_empty() && orphan_words.is_empty() {
            return;
        }
        ui.add_space(12.0);
        let t = folders::t();
        let title = (t.leftovers)(orphan_notes.len() + orphan_lists.len() + orphan_words.len());
        theme::collapsing(RichText::new(title).color(status_warn())).id_salt("folder-leftovers").show(ui, |ui| {
            ui.label(RichText::new(t.leftovers_hint).small().color(muted()));
            let mut remove_note = None;
            let name_w = self.name_w;
            for i in orphan_notes {
                ui.horizontal(|ui| {
                    name_column(ui, name_w, |ui| {
                        ui.add(egui::Label::new(RichText::new(&self.notes[i].0).color(status_warn())).truncate());
                    });
                    theme::field(
                        ui,
                        TextEdit::singleline(&mut self.notes[i].1).desired_width(ui.available_width() - 40.0),
                    );
                    if theme::ghost_button(ui, &theme::icon::X.to_string()).on_hover_text(t.remove_note).clicked() {
                        remove_note = Some(i);
                    }
                });
            }
            if let Some(i) = remove_note {
                self.notes.remove(i);
            }
            for p in orphan_words {
                ui.horizontal(|ui| {
                    name_column(ui, name_w, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(format!("{p} ({})", t.orphan_words)).color(status_warn()))
                                .truncate(),
                        );
                    });
                    let w = (ui.available_width() - 48.0).max(160.0);
                    words_field(ui, &mut self.words, &p, w);
                    if theme::ghost_button(ui, &theme::icon::X.to_string()).on_hover_text(t.remove_words).clicked() {
                        self.words.retain(|w| key(&w.path) != key(&p));
                    }
                });
            }
            for p in orphan_lists {
                ui.horizontal(|ui| {
                    let what = if Self::listed(&self.exclude, &p) { t.orphan_unused } else { t.orphan_collection };
                    ui.label(RichText::new(format!("{p} ({what})")).color(status_warn()));
                    if theme::ghost_button(ui, &theme::icon::X.to_string()).on_hover_text(t.remove_entry).clicked() {
                        Self::set_listed(&mut self.exclude, &p, false);
                        Self::set_listed(&mut self.collapse, &p, false);
                    }
                });
            }
        });
    }
}

/// Entry for a folder in a list (folder, text).
fn pair_index(list: &[(String, String)], path: &str) -> Option<usize> {
    let k = key(path);
    list.iter().position(|(f, _)| key(f) == k)
}

fn filled(list: &[(String, String)]) -> usize {
    list.iter().filter(|(_, v)| !v.trim().is_empty()).count()
}

/// Keywords and counter-words of a folder in one field, as in the configuration:
/// "word, word & word, -counterword".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WordsDraft {
    pub path: String,
    pub value: String,
}

impl WordsDraft {
    fn from_config(path: &str, value: &str) -> Self {
        let mut w = Self { path: path.to_string(), value: value.to_string() };
        w.value = w.to_config();
        w
    }

    /// Entries cleanly separated; "- word" becomes "-word".
    fn to_config(&self) -> String {
        items(&self.value)
            .into_iter()
            .map(|i| match i.strip_prefix('-') {
                Some(rest) => format!("-{}", rest.trim()),
                None => i,
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn is_empty(&self) -> bool {
        self.value.trim().is_empty()
    }
}

/// Entries of a word list (separated by comma, semicolon or line break).
fn items(s: &str) -> Vec<String> {
    s.split([',', ';', '\n']).map(str::trim).filter(|i| !i.is_empty()).map(String::from).collect()
}

/// A folder's keywords in words, the way Scrinium reads them – e.g.
/// "Matches "Haftpflicht" or "Police…" – not "Kontoauszug"".
fn meaning(w: &WordsDraft) -> Option<String> {
    let parsed = FolderWords::parse(&w.to_config());
    if parsed.is_empty() {
        return None;
    }
    let t = folders::t();
    let word = |s: &String| match s.strip_suffix('*') {
        Some(stem) => (t.quote)(&format!("{stem}…")),
        None => (t.quote)(s),
    };
    let groups = |v: &[Vec<String>]| {
        v.iter().map(|g| g.iter().map(word).collect::<Vec<_>>().join(t.word_and)).collect::<Vec<_>>().join(t.word_or)
    };
    let mut out = if parsed.plus.is_empty() { t.no_own_words.to_string() } else { (t.matches)(&groups(&parsed.plus)) };
    if !parsed.minus.is_empty() {
        out.push_str(&(t.not_counting)(&groups(&parsed.minus)));
    }
    Some(out)
}

/// The *Recognised by* field of a folder, with the fixed column width. On hover it shows how Scrinium
/// reads the words. Returns whether the field has focus.
fn words_field(ui: &mut egui::Ui, words: &mut Vec<WordsDraft>, path: &str, width: f32) -> bool {
    let index = words.iter().position(|w| key(&w.path) == key(path));
    let mut scratch = WordsDraft { path: path.to_string(), ..Default::default() };
    let entry = match index {
        Some(i) => &mut words[i],
        None => &mut scratch,
    };
    let help = folders::t().words_help;
    let hover = match meaning(entry) {
        Some(m) => format!("{m}\n\n{help}"),
        None => help.to_string(),
    };
    // No placeholder: in every empty row the example looked like content.
    let edit = TextEdit::singleline(&mut entry.value).desired_width(width - 2.0 * theme::FIELD_MARGIN);
    let resp = theme::field(ui, edit).on_hover_text(hover);
    if resp.changed() && index.is_none() && !scratch.is_empty() {
        words.push(scratch);
    }
    resp.has_focus()
}

/// Table row: a separator line at the bottom, `hover` under the pointer; the clicked row gets `highlight`
/// with an accent border (the fill alone barely stands out in dark mode, and secondary text sits on it).
fn table_row(ui: &mut egui::Ui, selected: bool, add: impl FnOnce(&mut egui::Ui)) {
    let mut row = egui::Frame::new().corner_radius(theme::radius::MD).inner_margin(Margin::symmetric(4, 2)).begin(ui);
    add(&mut row.content_ui);
    let hovered = ui.rect_contains_pointer(row.content_ui.min_rect().expand(4.0));
    row.frame.fill = match (selected, hovered) {
        (true, _) => tokens::highlight(),
        (false, true) => tokens::hover(),
        (false, false) => egui::Color32::TRANSPARENT,
    };
    let rect = row.end(ui).rect;
    if selected {
        let stroke = Stroke::new(1.0_f32, tokens::accent());
        ui.painter().rect_stroke(rect, theme::radius::MD, stroke, egui::StrokeKind::Inside);
    } else {
        ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0_f32, tokens::line()));
    }
}

fn name_column(ui: &mut egui::Ui, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(egui::vec2(width, ROW_HEIGHT), Layout::left_to_right(Align::Center), |ui| {
        ui.set_width(width);
        add(ui);
    });
}

/// Column headers above the overview; they stay in place when the rows scroll.
fn header(ui: &mut egui::Ui, name_w: f32, words_w: f32) {
    let t = folders::t();
    let head = theme::head_text;
    egui::Frame::new()
        .fill(tokens::surface_muted())
        .stroke(Stroke::new(1.0_f32, tokens::line()))
        .corner_radius(theme::radius::MD)
        .inner_margin(Margin::symmetric(4, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                name_column(ui, name_w, |ui| {
                    ui.add_space(18.0 + ui.spacing().item_spacing.x);
                    ui.label(head(t.col_folder));
                });
                let desc_w = (ui.available_width() - words_w - ui.spacing().item_spacing.x).max(120.0);
                ui.allocate_ui_with_layout(egui::vec2(desc_w, 18.0), Layout::left_to_right(Align::Center), |ui| {
                    ui.set_width(desc_w);
                    ui.label(head(t.col_description));
                });
                ui.label(head(t.col_words)).on_hover_text(t.words_help);
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn tree_hides_years_and_counts_them() {
        let tree = Tree::from_paths(
            "x",
            3,
            &paths(&[
                "Bank",
                "Bank/2025",
                "Bank/2026",
                "Bank/201x",
                "Bank/201x/2015",
                "Bank/Depot",
                "Haus",
                "Haus/_Strom",
                "Haus/_Strom/2026",
            ]),
        );
        let names: Vec<&str> = tree.nodes.iter().map(|n| n.path.as_str()).collect();
        assert_eq!(names, ["Bank", "Bank/Depot", "Haus", "Haus/_Strom"]);
        assert_eq!(tree.roots, [0, 2]);
        assert_eq!(tree.nodes[0].years, 3, "2025, 2026 and the bundle 201x");
        assert_eq!(tree.nodes[3].years, 1);
        assert_eq!(tree.nodes[0].children, [1]);
        assert_eq!(tree.nodes[0].series, ["{year}"]);
        assert!(tree.nodes[0].choosable_series().is_empty(), "the year logic picks a single series");

        let pay =
            Tree::from_paths("x", 3, &paths(&["Gehalt", "Gehalt/2025_Max", "Gehalt/2026_Max", "Gehalt/2026_Erika"]));
        assert_eq!(pay.nodes[0].choosable_series(), ["{year}_Max", "{year}_Erika"]);
        assert!(pay.has_series("gehalt/{year}_max") && !pay.has_series("Gehalt/{year}_Moritz"));
        assert!(tree.known.contains("bank/201x/2015"));
    }

    #[test]
    fn exclude_toggles_and_old_collections_are_taken_over() {
        let mut cfg = Config {
            exclude: vec!["Alt".into()],
            collapse: vec!["Wissen".into(), "Fehlt".into()],
            ..Default::default()
        };
        cfg.folder_notes.insert("Haus".into(), "Haus und Garten".into());
        let mut view = FolderView::new(&cfg);
        assert!(FolderView::listed(&view.exclude, "alt"));
        FolderView::set_listed(&mut view.exclude, "Alt", false);
        FolderView::set_listed(&mut view.exclude, "Haus/Garten", true);
        FolderView::set_listed(&mut view.exclude, "haus/garten", true);
        assert_eq!(view.exclude, ["haus/garten"]);
        assert!(FolderView::ancestor_in(&view.exclude, "Haus/Garten/Teich"));

        // Collection folder "Wissen": its subfolders become unused, year folders stay.
        view.tree = Tree::from_paths("x", 3, &paths(&["Wissen", "Wissen/Rust", "Wissen/Go", "Wissen/2026"]));
        view.take_over_collections();
        assert_eq!(view.migrated, ["Wissen"]);
        assert!(FolderView::listed(&view.exclude, "Wissen/Rust") && FolderView::listed(&view.exclude, "Wissen/Go"));
        assert!(!FolderView::listed(&view.exclude, "Wissen/2026"));
        assert_eq!(view.collapse, ["Fehlt"]);

        view.notes.push(("Leer".into(), "  ".into()));
        view.words.push(WordsDraft::from_config("Haus", " Grundsteuer & Musterstadt , - Kontoauszug"));
        view.words.push(WordsDraft::from_config("Leer", ""));
        let mut out = Config::default();
        view.apply(&mut out);
        assert_eq!(out.folder_notes.len(), 1, "empty descriptions are dropped");
        assert_eq!(out.folder_words.get("Haus").map(String::as_str), Some("Grundsteuer & Musterstadt, -Kontoauszug"));
        assert_eq!(out.folder_words.len(), 1);
        let haus = view.words_of("haus").expect("keywords");
        assert_eq!(haus.value, "Grundsteuer & Musterstadt, -Kontoauszug");
        assert!(view.words_of("Leer").is_none());

        // A field as in the configuration; a semicolon separates too, "- word" becomes "-word".
        let w = WordsDraft { path: "X".into(), value: "Haftpflicht; Police*,, - Kontoauszug, -Umsatz & Bank".into() };
        assert_eq!(w.to_config(), "Haftpflicht, Police*, -Kontoauszug, -Umsatz & Bank");
        assert_eq!(out.collapse, ["Fehlt"]);
    }

    #[test]
    fn words_are_explained_and_fallback_checked() {
        let w = WordsDraft {
            path: "X".into(),
            value: "Haftpflicht, Grundsteuer & Musterstadt, Versicherungs*, -Kontoauszug".into(),
        };
        assert_eq!(
            meaning(&w).as_deref(),
            Some(
                "Trifft bei „Haftpflicht“ oder „Grundsteuer“ und „Musterstadt“ oder „Versicherungs…“ – Stichwörter zählen nicht bei „Kontoauszug“"
            )
        );
        assert!(meaning(&WordsDraft::default()).is_none());

        let mut cfg = Config { fallback_folder: "Eingang".into(), ..Default::default() };
        let mut view = FolderView::new(&cfg);
        assert!(view.fallback_problem(&cfg).is_none(), "no verdict before the scan");
        view.tree = Tree::from_paths("x", 3, &paths(&["Eingang", "Alt", "Alt/Kiste"]));
        assert!(view.fallback_problem(&cfg).is_none());
        assert_eq!(view.usable(), ["Eingang", "Alt", "Alt/Kiste"]);
        FolderView::set_listed(&mut view.exclude, "Alt", true);
        assert_eq!(view.usable(), ["Eingang"]);
        cfg.fallback_folder = "Alt/Kiste".into();
        assert!(view.fallback_problem(&cfg).is_some_and(|p| p.contains("nicht verwendet")));
        cfg.fallback_folder = "Gibts/Nicht".into();
        assert!(view.fallback_problem(&cfg).is_some_and(|p| p.contains("gibt es nicht")));
    }
}
