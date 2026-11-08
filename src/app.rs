use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, ColorImage, FontId, Id, Key, LayerId, Layout, Margin, Order, RichText, ScrollArea,
    Stroke, TextureHandle, TextureOptions, Vec2,
};

use crate::about::{self, GeneralAction};
use crate::analysis::{self, Analysis, Event, Job, Origin, SetupJob, Suggestion, Work};
use crate::archive::{self, FileRequest, Filed, FolderIndex};
use crate::config::{self, Config, LlmBackend, LlmConfig, ScanPreset, config_path};
use crate::form::{self, Form, FormAction, FormCtx};
use crate::heuristics;
use crate::i18n::window;
use crate::ingest::{self, DocKind};
use crate::journal;
use crate::llm::LlmClient;
use crate::llm_ui::{self, LocalAi};
use crate::pages::{PageCache, Slot};
use crate::rotate;
use crate::scan;
use crate::scan_pdf;
use crate::settings::{self, Leave, SettingsAction, SettingsDraft, Tab};
use crate::setup;
use crate::tags::{self, Outcome};
use crate::theme::{self, Arrow, icon, text, tokens};
use crate::update::Updates;
use tokens::{accent, muted, status_err, status_ok, status_warn};

#[derive(Clone)]
enum LlmStatus {
    Disabled,
    Checking,
    Online { model: String, detail: String },
    Offline(String),
}

enum DocStatus {
    Queued,
    Working(String),
    Ready,
    Filed(Filed),
    Failed(String),
}

struct DocItem {
    id: u64,
    path: PathBuf,
    name: String,
    status: DocStatus,
    analysis: Option<Box<Analysis>>,
    form: Form,
    /// Pages of the preview: those of the analysis and those loaded later while paging.
    pages: PageCache,
    /// Scanned in Scrinium: always moved on filing, never copied.
    scanned: bool,
    page: usize,
    /// Already handed to the analysis thread. Only one document runs at a time.
    submitted: bool,
    /// Error of the last filing attempt; shown in the form until the next attempt.
    file_error: Option<String>,
    /// Zoom of the preview (1 = whole page).
    zoom: f32,
    /// Manually chosen rotation per page (clockwise, 0/90/180/270), on top of the detected one.
    turns: Vec<u16>,
    /// Rotation per page that the file already contains: it was rotated on filing and lies at the target that way,
    /// and after an undo back at the old place. Pages loaded later are turned back by this much, and the next filing
    /// adds only the difference.
    baked: Vec<u16>,
    /// Start of the analysis, for showing the elapsed time.
    started: Option<Instant>,
}

/// Last filed document: enough to undo it even when it is no longer in the list.
struct LastFiled {
    id: u64,
    name: String,
    filed: Filed,
}

impl DocItem {
    fn is_waiting(&self) -> bool {
        !self.submitted && matches!(self.status, DocStatus::Queued)
    }

    fn is_running(&self) -> bool {
        self.submitted && matches!(self.status, DocStatus::Queued | DocStatus::Working(_))
    }
}

struct Toast {
    text: String,
    error: bool,
    since: Instant,
}

pub struct ScriniumApp {
    config: Config,
    config_path: PathBuf,
    folders: FolderIndex,
    docs: Vec<DocItem>,
    selected: Option<u64>,
    next_id: u64,
    job_tx: Sender<Work>,
    /// Requests for the quick preview (page images without analysis).
    preview_tx: Sender<analysis::PreviewJob>,
    /// *Set up with AI* tab; kept when the settings are closed.
    setup: setup::Panel,
    events: Receiver<Event>,
    llm_status: LlmStatus,
    llm_rx: Option<Receiver<LlmStatus>>,
    settings: Option<SettingsDraft>,
    toast: Option<Toast>,
    folder_filter: String,
    /// Pauses the queue: the running document finishes, the next one does not start.
    queue_paused: bool,
    local_ai: LocalAi,
    /// Last filed document, for *Undo* in the status bar.
    last_filed: Option<LastFiled>,
    /// Confirmation before edits are discarded (remove, analyze again).
    pending: Option<(u64, Action)>,
    /// Queue on the left hidden (more room for document and form).
    queue_hidden: bool,
    /// The window may close (the question about unsaved settings is answered).
    allow_close: bool,
    /// Chosen appearance (system, light, dark).
    mode: theme::Mode,
    /// Since when the shown document has been shown: Ctrl+Enter works only after a short pause, so that a held
    /// Ctrl+Enter does not file the next document as well.
    shown: Option<u64>,
    shown_since: Instant,
    /// Check for new versions and the info window.
    updates: Updates,
    /// Is a scan running (the scanner's dialog is open)?
    scan_rx: Option<Receiver<scan::ScanResult>>,
    /// Pages scanned so far; *Done* turns them into a PDF.
    scan_pages: Vec<scan::ScannedPage>,
    /// The last scanned page scaled down and its preview in the chosen color mode.
    /// A scan is being prepared and saved; `scan_step` tells which step.
    scan_job: Option<Receiver<scan_pdf::Progress>>,
    scan_step: String,
    /// How far the preparation is (0 to 1), for the progress pill.
    scan_share: f32,
}

impl ScriniumApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_files: Vec<PathBuf>) -> Self {
        let config_path = config_path();
        let mode = theme::Mode::load(config_path.parent().unwrap_or(Path::new(".")));
        let updates = Updates::load(config_path.parent().unwrap_or(Path::new(".")));
        let queue_hidden = theme::read_view(config_path.parent().unwrap_or(Path::new("."))).list == Some(false);
        theme::apply(&cc.egui_ctx, mode);
        let (config, toast) = match Config::load_or_create(&config_path) {
            Ok(c) => (c, None),
            Err(e) => (
                Config::default(),
                Some(Toast {
                    text: (window::t().defaults_active)(&format!("{e:#}")),
                    error: true,
                    since: Instant::now(),
                }),
            ),
        };
        let ctx = cc.egui_ctx.clone();
        let (job_tx, preview_tx, events) = analysis::spawn_worker(move || ctx.request_repaint());
        let mut app = Self {
            folders: FolderIndex::default(),
            config,
            config_path,
            docs: Vec::new(),
            selected: None,
            next_id: 1,
            job_tx,
            preview_tx,
            events,
            llm_status: LlmStatus::Disabled,
            llm_rx: None,
            settings: None,
            toast,
            folder_filter: String::new(),
            queue_paused: false,
            local_ai: LocalAi::default(),
            last_filed: None,
            pending: None,
            queue_hidden,
            setup: setup::Panel::default(),
            allow_close: false,
            mode,
            shown: None,
            shown_since: Instant::now(),
            updates,
            scan_rx: None,
            scan_pages: Vec::new(),
            scan_job: None,
            scan_step: String::new(),
            scan_share: 0.0,
        };
        app.rescan_folders();
        app.check_llm(&cc.egui_ctx);
        app.add_files(initial_files);
        // For screenshots and tests: SCRINIUM_SETTINGS=archive|names|rules|ai|scan|general|setup opens the settings
        // on this tab right away.
        if let Ok(tab) = std::env::var("SCRINIUM_SETTINGS") {
            app.open_settings(match tab.as_str() {
                "names" => Tab::Names,
                "rules" => Tab::Rules,
                "ai" => Tab::Ai,
                "scan" => Tab::Scan,
                "setup" => Tab::Setup,
                "general" => Tab::General,
                _ => Tab::Archive,
            });
        }
        app
    }

    fn rescan_folders(&mut self) {
        let c = &self.config;
        self.folders = FolderIndex::scan(&c.archive_root(), c.scan_depth, &c.exclude, &c.collapse);
    }

    fn check_llm(&mut self, ctx: &egui::Context) {
        if !self.config.llm.enabled {
            self.llm_status = LlmStatus::Disabled;
            self.llm_rx = None;
            return;
        }
        self.llm_status = LlmStatus::Checking;
        let (tx, rx) = channel();
        self.llm_rx = Some(rx);
        let cfg = self.config.llm.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let status = match cfg.backend {
                LlmBackend::Embedded => match llm_ui::probe(&cfg) {
                    Ok((model, detail)) => LlmStatus::Online { model, detail },
                    Err(e) => LlmStatus::Offline(e),
                },
                LlmBackend::Server => probe_llm(&cfg),
            };
            let _ = tx.send(status);
            ctx.request_repaint();
        });
    }

    fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.toast = Some(Toast { text: text.into(), error, since: Instant::now() });
    }

    /// Check for new versions: collect the answer, report a new version once, and on first start ask whether
    /// Scrinium may check (not on top of another question).
    fn updates_ui(&mut self, ctx: &egui::Context) {
        self.updates.poll(ctx);
        if let Some(v) = self.updates.to_tell.take() {
            self.notify((crate::i18n::general::t().update_available)(&v.to_string()), false);
        }
        if self.pending.is_none()
            && self.updates.should_ask(ctx)
            && let Some(yes) = about::question(ctx)
        {
            self.updates.set_enabled(yes);
        }
    }

    /// Adds files and folders (one level deep) to the queue; tells what was skipped.
    fn add_files(&mut self, paths: Vec<PathBuf>) {
        let mut files = Vec::new();
        let mut subfolders = 0;
        for p in paths {
            if p.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&p) {
                    let mut inner: Vec<PathBuf> = Vec::new();
                    for e in entries.filter_map(|e| e.ok().map(|e| e.path())) {
                        if e.is_file() {
                            inner.push(e);
                        } else if e.is_dir() {
                            subfolders += 1;
                        }
                    }
                    inner.sort();
                    files.extend(inner);
                }
            } else {
                files.push(p);
            }
        }
        let (mut added, mut skipped, mut twice) = (0, 0, 0);
        let mut first_name = String::new();
        for path in files {
            if !ingest::is_supported(&path) {
                skipped += 1;
                continue;
            }
            let already = self
                .docs
                .iter()
                .any(|d| d.path == path && !matches!(d.status, DocStatus::Filed(_) | DocStatus::Failed(_)));
            if already {
                twice += 1;
                continue;
            }
            let id = self.next_id;
            self.next_id += 1;
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if first_name.is_empty() {
                first_name = name.clone();
            }
            self.docs.push(DocItem {
                id,
                path,
                name,
                status: DocStatus::Queued,
                analysis: None,
                form: Form::default(),
                pages: PageCache::default(),
                scanned: false,
                page: 0,
                submitted: false,
                file_error: None,
                zoom: 1.0,
                turns: Vec::new(),
                baked: Vec::new(),
                started: None,
            });
            added += 1;
            if self.selected.is_none() {
                self.selected = Some(id);
            }
        }
        let t = window::t();
        let mut parts = Vec::new();
        if added > 0 {
            parts.push((t.queued)(added, &first_name));
        }
        if skipped > 0 {
            parts.push((t.skipped)(skipped));
        }
        if twice > 0 {
            parts.push((t.already_listed)(twice));
        }
        if subfolders > 0 {
            parts.push((t.subfolders_left_out)(subfolders));
        }
        if !parts.is_empty() {
            self.notify(parts.join(" · "), added == 0);
        }
    }

    fn enqueue(&mut self, id: u64) {
        let Some(doc) = self.docs.iter_mut().find(|d| d.id == id) else { return };
        doc.status = DocStatus::Queued;
        doc.submitted = false;
    }

    /// Sends the next waiting document to the analysis thread as soon as none is running.
    fn pump_queue(&mut self) {
        if self.queue_paused || self.docs.iter().any(DocItem::is_running) {
            return;
        }
        let Some(doc) = self.docs.iter_mut().find(|d| d.is_waiting()) else { return };
        doc.submitted = true;
        doc.started = Some(Instant::now());
        let job =
            Job { id: doc.id, path: doc.path.clone(), config: self.config.clone(), folders: self.folders.clone() };
        if self.job_tx.send(Work::Analyse(job)).is_err() {
            doc.submitted = false;
            doc.status = DocStatus::Failed(window::t().worker_stopped.into());
        }
    }

    fn move_in_queue(&mut self, id: u64, delta: isize) {
        let Some(i) = self.docs.iter().position(|d| d.id == id) else { return };
        if !self.docs[i].is_waiting() {
            return;
        }
        let mut j = i as isize;
        loop {
            j += delta;
            if j < 0 || j as usize >= self.docs.len() {
                return;
            }
            if self.docs[j as usize].is_waiting() {
                break;
            }
        }
        self.docs.swap(i, j as usize);
    }

    /// Position in the analysis queue (running and waiting), 1-based.
    fn queue_slot(&self, id: u64) -> Option<(usize, usize)> {
        let ids: Vec<u64> = self
            .docs
            .iter()
            .filter(|d| matches!(d.status, DocStatus::Queued | DocStatus::Working(_)))
            .map(|d| d.id)
            .collect();
        ids.iter().position(|x| *x == id).map(|i| (i + 1, ids.len()))
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        while let Ok(ev) = self.events.try_recv() {
            match ev {
                Event::Progress { id, message } => {
                    if let Some(d) = self.docs.iter_mut().find(|d| d.id == id) {
                        d.status = DocStatus::Working(message);
                    }
                }
                Event::Failed { id, error } => {
                    if let Some(d) = self.docs.iter_mut().find(|d| d.id == id) {
                        d.status = DocStatus::Failed(error);
                    }
                }
                // Pages loaded later; they do not replace the (rotated) pages of the analysis (`PageCache::set_page`).
                Event::Preview { id, page_count, pages } => {
                    if let Some(d) = self.docs.iter_mut().find(|d| d.id == id) {
                        let pages = (pages.into_iter())
                            .map(|(i, page)| match page {
                                Ok(p) => (i, Slot::Loaded(page_texture(ctx, id, i, &p))),
                                Err(e) => (i, Slot::Failed(e)),
                            })
                            .collect();
                        d.pages.receive(page_count, pages);
                    }
                }
                Event::SetupProgress(message) => self.setup.status = message,
                Event::SetupCancelled => {}
                Event::SetupDone(result) => {
                    self.setup.running = false;
                    self.setup.cancel = None;
                    match result {
                        Ok(p) => {
                            // Existing special rules and instructions are replaced only when explicitly chosen.
                            let current = self.settings.as_ref().map_or_else(|| self.config.clone(), |d| d.build());
                            self.setup.take = setup::Sections {
                                rules: current.rules.is_empty(),
                                instructions: current.llm.instructions.trim().is_empty(),
                                ..Default::default()
                            };
                            self.setup.proposal = Some(*p);
                            self.notify(window::t().setup_ready, false);
                        }
                        Err(e) => self.setup.error = Some(e),
                    }
                }
                Event::Done { id, analysis } => {
                    let cfg = &self.config;
                    if let Some(d) = self.docs.iter_mut().find(|d| d.id == id) {
                        // The pages of the analysis are rotated and deskewed; they replace the first pages of the
                        // preview, the others stay. A manual rotation applied to the unrotated page, so it starts
                        // over on the pages of the analysis; on the others it stays.
                        let analysed = analysis.previews.len();
                        if d.analysis.is_none() {
                            d.turns.iter_mut().take(analysed).for_each(|t| *t = 0);
                        }
                        // The analysis read the file as it is now; any rotation it already contains is part of the
                        // detected one.
                        d.baked.iter_mut().take(analysed).for_each(|t| *t = 0);
                        d.pages.set_analysed(page_textures(ctx, id, &analysis.previews), analysis.page_count);
                        set_form(d, analysis, cfg);
                    }
                }
            }
        }
        if let Some(rx) = &self.llm_rx
            && let Ok(status) = rx.try_recv()
        {
            self.llm_status = status;
            self.llm_rx = None;
        }
        if let Some((text, error)) = self.local_ai.poll() {
            self.notify(text, error);
            self.check_llm(ctx);
        }
        if self.local_ai.download_status().is_some() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    fn confirm(&mut self, id: u64) {
        let root = self.config.archive_root();
        let Some(doc) = self.docs.iter_mut().find(|d| d.id == id) else { return };
        // A scan is always moved: it should no longer be in the scan folder afterwards.
        let copy = self.config.copy_instead_of_move && !doc.scanned;
        doc.file_error = None;
        let result = archive::file_document(&FileRequest {
            source: &doc.path,
            archive_root: &root,
            folder: &doc.form.folder,
            filename_stem: &doc.form.filename,
            copy,
        });
        let t = window::t();
        match result {
            Ok(filed) => {
                let shown = relative(&root, &filed.target);
                // The pages as the preview shows them – before the tags that are appended to the file.
                let turns = missing_turns(doc);
                let turn_note = if rotate::any(&turns) {
                    match rotate::apply(&filed.target, &turns) {
                        Ok(Outcome::Written) => {
                            // The copy is rotated, the original stays as it was.
                            if !copy {
                                doc.baked = page_turns(doc);
                            }
                            t.pages_turned.to_string()
                        }
                        Ok(Outcome::Unsupported) => t.turn_unsupported.to_string(),
                        Ok(Outcome::Skipped(why)) => (t.turn_skipped)(&why),
                        Err(e) => (t.turn_failed)(&format!("{e:#}")),
                    }
                } else {
                    String::new()
                };
                let tag_note = if self.config.write_tags {
                    match tags::write(&filed.target, &search_tags(doc)) {
                        Ok(Outcome::Written) => t.tags_written.to_string(),
                        Ok(Outcome::Unsupported) => String::new(),
                        Ok(Outcome::Skipped(why)) => (t.tags_skipped)(&why),
                        Err(e) => (t.tags_failed)(&format!("{e:#}")),
                    }
                } else {
                    String::new()
                };
                let journal_note = log_filed(&self.config, doc, &filed.target);
                let name = doc.name.clone();
                self.last_filed = Some(LastFiled { id, name, filed: filed.clone() });
                doc.status = DocStatus::Filed(filed);
                self.notify(format!("{}{turn_note}{tag_note}{journal_note}", (t.filed_toast)(&shown)), false);
                self.rescan_folders();
                self.select_next_after(id);
            }
            Err(e) => {
                doc.file_error = Some(format!("{e:#}"));
                self.notify(t.file_failed, true);
            }
        }
    }

    /// Moves the file back to the old place (for a copy: removes the copy); the document is then open again.
    fn undo(&mut self, id: u64) {
        let Some(doc) = self.docs.iter_mut().find(|d| d.id == id) else { return };
        let DocStatus::Filed(filed) = &doc.status else { return };
        let copied = filed.copied;
        let t = window::t();
        match archive::undo_filing(filed) {
            Ok(()) => {
                let journal_note = log_undo(&self.config, &filed.target);
                let name = doc.name.clone();
                doc.status = DocStatus::Ready;
                self.selected = Some(id);
                if self.last_filed.as_ref().is_some_and(|l| l.id == id) {
                    self.last_filed = None;
                }
                let done = if copied { t.undo_copy_removed } else { t.undo_moved_back };
                self.notify(format!("{}{journal_note}", done(&name)), false);
                self.rescan_folders();
            }
            Err(e) => self.notify((t.undo_failed)(&format!("{e:#}")), true),
        }
    }

    /// Undoes the last filing – even when the document was already taken off the list; it then goes into the
    /// queue again.
    fn undo_last(&mut self) {
        let Some(last) = self.last_filed.as_ref() else { return };
        let id = last.id;
        if self.docs.iter().any(|d| d.id == id && matches!(d.status, DocStatus::Filed(_))) {
            self.undo(id);
            return;
        }
        let (name, filed) = (last.name.clone(), last.filed.clone());
        let t = window::t();
        match archive::undo_filing(&filed) {
            Ok(()) => {
                let journal_note = log_undo(&self.config, &filed.target);
                self.last_filed = None;
                self.rescan_folders();
                if filed.copied {
                    self.notify(format!("{}{journal_note}", (t.undo_last_copy_removed)(&name)), false);
                } else {
                    self.add_files(vec![filed.source]);
                    self.notify(format!("{}{journal_note}", (t.undo_last_moved_back)(&name)), false);
                }
            }
            Err(e) => self.notify((t.undo_failed)(&format!("{e:#}")), true),
        }
    }

    /// File anyway after a failed analysis: form with the date from the file name.
    fn file_manually(&mut self, id: u64) {
        let cfg = &self.config;
        let Some(d) = self.docs.iter_mut().find(|d| d.id == id) else { return };
        let error = match &d.status {
            DocStatus::Failed(e) => e.clone(),
            _ => return,
        };
        let stem = analysis::original_stem(&d.path);
        let ext = ingest::extension(&d.path).unwrap_or_default();
        let suggestion = Suggestion {
            date: heuristics::filename_date(&stem),
            folder: cfg.fallback_folder.clone(),
            source: "manual".into(),
            keep_name: crate::naming::keeps_name(&ext, &cfg.keep_name_extensions),
            ..Default::default()
        };
        let a = Analysis {
            suggestion,
            kind: DocKind::of(&d.path),
            text: String::new(),
            previews: Vec::new(),
            page_count: 0,
            ocr_engine: None,
            corrections: Vec::new(),
            rotations: Vec::new(),
            warnings: vec![(window::t().analysis_failed_warning)(&error)],
            duration: Duration::ZERO,
            ai_stats: None,
        };
        set_form(d, Box::new(a), cfg);
    }

    /// After filing or *Later*: the next document ready for review, otherwise the next open one.
    fn select_next_after(&mut self, id: u64) {
        let pos = self.docs.iter().position(|d| d.id == id).unwrap_or(0);
        let order: Vec<&DocItem> = self.docs[pos + 1..].iter().chain(self.docs[..pos].iter()).collect();
        let next = order
            .iter()
            .find(|d| matches!(d.status, DocStatus::Ready))
            .or_else(|| order.iter().find(|d| !matches!(d.status, DocStatus::Filed(_))));
        if let Some(n) = next {
            self.selected = Some(n.id);
        }
    }

    /// Ctrl+Page Up/Down: previous or next document; filed ones are skipped.
    fn select_relative(&mut self, delta: isize) {
        let Some(pos) = self.selected.and_then(|id| self.docs.iter().position(|d| d.id == id)) else { return };
        let mut j = pos as isize;
        loop {
            j += delta;
            if j < 0 || j as usize >= self.docs.len() {
                return;
            }
            if !matches!(self.docs[j as usize].status, DocStatus::Filed(_)) {
                self.selected = Some(self.docs[j as usize].id);
                return;
            }
        }
    }

    fn remove(&mut self, id: u64) {
        self.docs.retain(|d| d.id != id);
        if self.selected == Some(id) {
            self.selected = self.docs.first().map(|d| d.id);
        }
    }

    /// Scanning: the scanner's dialog runs in the background, each page arrives in `scan_ui`.
    fn start_scan(&mut self, ctx: &egui::Context) {
        if self.scan_rx.is_some() {
            return;
        }
        self.scan_rx = Some(scan::start(ctx, config::cache_dir().join("scan"), self.config.scan.clone()));
        self.notify(window::t().scanner_opening, false);
    }

    /// New scans go into the scan folder (default: Downloads); filing moves them from there.
    fn scan_dir(&self) -> PathBuf {
        self.config.scan.folder_path()
    }

    /// When the scanned pages arrive, they are prepared in the chosen color mode without asking and put into the
    /// list as a PDF. The color mode sits next to the *Scan* button.
    fn scan_ui(&mut self, ctx: &egui::Context) {
        self.poll_scan_job();
        let t = window::t();
        let Some(rx) = &self.scan_rx else { return };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(t.scan_cancelled.into()),
        };
        self.scan_rx = None;
        match result {
            Ok(pages) if !pages.is_empty() => {
                self.scan_pages = pages;
                self.finish_scan(ctx);
            }
            Ok(_) => {}
            Err(e) => self.notify((t.scan_error)(&e), true),
        }
    }

    /// Color mode for the next scans; saved right away unless the settings are open.
    fn set_scan_preset(&mut self, preset: ScanPreset) {
        if self.config.scan.preset == preset {
            return;
        }
        self.config.scan.preset = preset;
        if self.settings.is_none()
            && let Err(e) = self.config.save(&self.config_path)
        {
            self.notify(format!("{e:#}"), true);
        }
    }

    /// Row "Scan as [type ▾]", right-aligned below the *Scan* button; the combo box is `width` wide like the button.
    /// In the menu each type has a line on what it does. Call at the start of an area as wide as the button row.
    fn scan_as_row(&mut self, ui: &mut egui::Ui, width: f32) {
        let t = window::t();
        let mut chosen = self.config.scan.preset;
        // Exactly one row high; without a fixed height the right-to-left layout would take all the remaining space.
        let row = Vec2::new(ui.available_width(), theme::CONTROL);
        ui.allocate_ui_with_layout(row, Layout::right_to_left(Align::Center), |ui| {
            let resp = theme::combo("scan-as").selected_text(chosen.label()).width(width).show_ui(ui, |ui| {
                ui.set_min_width(300.0);
                for p in ScanPreset::ALL {
                    let mut job = egui::text::LayoutJob::default();
                    let body = egui::TextFormat::simple(FontId::proportional(text::BODY), tokens::fg());
                    let small = egui::TextFormat::simple(FontId::proportional(text::SMALL), muted());
                    job.append(p.label(), 0.0, body);
                    job.append(&format!("\n{}", p.hint()), 0.0, small);
                    ui.selectable_value(&mut chosen, p, job);
                }
            });
            resp.response.on_hover_text(chosen.hint());
            ui.label(RichText::new(t.scan_as).color(muted()));
        });
        self.set_scan_preset(chosen);
    }

    /// Prepares the scanned pages in the background and puts them into the scan folder as a PDF.
    fn finish_scan(&mut self, ctx: &egui::Context) {
        let dir = self.scan_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            // The pages stay, *Done* can be tried again.
            let text = (window::t().scan_dir_failed)(&dir.display().to_string(), &e.to_string());
            self.notify(text, true);
            return;
        }
        let name = scan::file_name(chrono::Local::now().naive_local());
        let mut path = dir.join(&name);
        for n in 2.. {
            if !path.exists() {
                break;
            }
            path = dir.join(name.replace(".pdf", &format!("_{n}.pdf")));
        }
        let job = scan_pdf::Job {
            pages: std::mem::take(&mut self.scan_pages),
            preset: self.config.scan.preset,
            text_layer: self.config.scan.text_layer,
            ocr: self.config.ocr.clone(),
            path,
        };
        self.scan_step = window::t().scan_preparing.into();
        self.scan_share = 0.0;
        self.scan_job = Some(scan_pdf::start(ctx, job));
    }

    /// Progress of the preparation; once the PDF is saved, it goes into the list.
    fn poll_scan_job(&mut self) {
        let Some(rx) = &self.scan_job else { return };
        let t = window::t();
        let done = loop {
            match rx.try_recv() {
                Ok(scan_pdf::Progress::Step(step, share)) => {
                    self.scan_step = step;
                    if let Some(share) = share {
                        self.scan_share = share;
                    }
                }
                Ok(done) => break done,
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    break scan_pdf::Progress::Failed { error: t.preparation_cancelled.into() };
                }
            }
        };
        self.scan_job = None;
        match done {
            scan_pdf::Progress::Saved { path, notes } => {
                let shown = path.display().to_string();
                self.add_files(vec![path.clone()]);
                if let Some(d) = self.docs.iter_mut().find(|d| d.path == path) {
                    d.scanned = true;
                }
                match notes.is_empty() {
                    true => self.notify((t.scan_saved)(&shown), false),
                    false => self.notify(format!("{} – {}", (t.scan_saved)(&shown), notes.join("; ")), false),
                }
            }
            // Without a confirmation step there is no dialog the pages could return to: scan again.
            scan_pdf::Progress::Failed { error } => self.notify((t.scan_not_saved)(&error), true),
            scan_pdf::Progress::Step(..) => {}
        }
    }

    /// Discards the suggestion and puts the document back into the queue. The pages stay in the preview until the
    /// new analysis is done.
    fn reanalyze(&mut self, id: u64) {
        if let Some(d) = self.docs.iter_mut().find(|d| d.id == id) {
            d.analysis = None;
        }
        self.enqueue(id);
    }

    /// Preview for the shown document: page images as soon as it is selected, not only after the analysis, and
    /// while paging the remaining pages, also beyond those of the analysis. Far-away pages and those of other
    /// documents are freed. Text and Office files show their content only with the suggestion.
    fn request_preview(&mut self) {
        for d in &mut self.docs {
            let shown = self.selected == Some(d.id);
            let current = d.page.min(d.pages.count().saturating_sub(1));
            d.pages.trim(current, shown);
            let wanted = d.pages.wanted(current);
            if !shown
                || wanted.is_empty()
                || matches!(d.status, DocStatus::Failed(_))
                || !matches!(DocKind::of(&d.path), DocKind::Pdf | DocKind::Image)
            {
                continue;
            }
            // Filed and moved: the file now lies at the target.
            let path = match &d.status {
                DocStatus::Filed(filed) if !filed.copied => filed.target.clone(),
                _ => d.path.clone(),
            };
            d.pages.loading = true;
            let job = analysis::PreviewJob { id: d.id, path, pages: wanted, turned: d.baked.clone() };
            if self.preview_tx.send(job).is_err() {
                d.pages.loading = false;
            }
        }
    }

    fn open_settings(&mut self, tab: Tab) {
        // Already open: only switch the tab, changes stay.
        if let Some(draft) = &mut self.settings {
            draft.tab = tab;
            return;
        }
        let mut draft = SettingsDraft::new(&self.config, tab);
        draft.opened = config::modified(&self.config_path);
        // An earlier "applied" state does not carry over to a new draft.
        self.setup.applied = None;
        if let Some(a) =
            self.selected.and_then(|id| self.docs.iter().find(|d| d.id == id)).and_then(|d| d.analysis.as_ref())
        {
            draft.set_probe(&a.text);
        }
        self.settings = Some(draft);
    }

    /// Back to the documents; with unsaved changes only after confirmation.
    fn leave_settings(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.settings.as_mut().and_then(|d| d.request_leave(Leave::Back)) {
            self.apply_settings(ctx, action);
        }
    }

    fn apply_settings(&mut self, ctx: &egui::Context, action: SettingsAction) {
        match action {
            SettingsAction::General(GeneralAction::SetMode(mode)) => {
                self.mode = mode;
                theme::set_mode(ctx, mode);
                mode.save(self.config_path.parent().unwrap_or(Path::new(".")));
            }
            SettingsAction::General(GeneralAction::SetLanguage(lang)) => {
                crate::i18n::choose(self.config_path.parent().unwrap_or(Path::new(".")), lang);
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(crate::i18n::general::t().window_title.into()));
                // The AI status is already text; check again so that it shows in the new language.
                self.check_llm(ctx);
            }
            SettingsAction::General(GeneralAction::CheckNow) => self.updates.check_now(ctx),
            SettingsAction::General(GeneralAction::SetUpdates(on)) => self.updates.set_enabled(on),
            SettingsAction::Cancel => {
                self.settings = None;
                self.setup.applied = None;
            }
            SettingsAction::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            SettingsAction::GenerateSetup => {
                let Some(draft) = &self.settings else { return };
                let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let job = SetupJob {
                    config: draft.build(),
                    description: self.setup.description.clone(),
                    folders: self.folders.clone(),
                    cancel: cancel.clone(),
                };
                self.setup.cancel = Some(cancel);
                self.setup.running = true;
                let t = window::t();
                self.setup.status = t.setup_waiting.into();
                self.setup.error = None;
                self.setup.applied = None;
                if self.job_tx.send(Work::Setup(job)).is_err() {
                    self.setup.running = false;
                    self.setup.error = Some(t.worker_stopped.into());
                }
            }
            SettingsAction::Reload => match Config::load_or_create(&self.config_path) {
                Ok(c) => {
                    let tab = self.settings.as_ref().map(|s| s.tab).unwrap_or_default();
                    let mut draft = SettingsDraft::new(&c, tab);
                    draft.opened = config::modified(&self.config_path);
                    self.settings = Some(draft);
                    self.set_config(ctx, c);
                    self.notify(window::t().config_reloaded, false);
                }
                Err(e) => self.notify(format!("{e:#}"), true),
            },
            SettingsAction::Save | SettingsAction::SaveAnyway | SettingsAction::SaveAndQuit => {
                let Some(draft) = self.settings.as_mut() else { return };
                // If the file was changed outside since opening, ask first.
                let changed_outside = config::modified(&self.config_path) != draft.opened;
                if changed_outside && !matches!(action, SettingsAction::SaveAnyway) {
                    draft.conflict = true;
                    return;
                }
                let cfg = draft.build();
                // If saving fails, the draft stays open and the error is shown next to the *Save* button.
                if let Err(e) = cfg.save(&self.config_path) {
                    draft.save_error = Some(format!("{e:#}"));
                    return;
                }
                self.settings = None;
                self.notify(window::t().settings_saved, false);
                self.set_config(ctx, cfg);
                if matches!(action, SettingsAction::SaveAndQuit) {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn set_config(&mut self, ctx: &egui::Context, cfg: Config) {
        let llm_changed = cfg.llm != self.config.llm;
        self.config = cfg;
        self.rescan_folders();
        if llm_changed {
            self.check_llm(ctx);
        }
        let cfg = &self.config;
        for d in &mut self.docs {
            if matches!(d.status, DocStatus::Ready) && !d.form.filename_manual {
                let keep = d.analysis.as_ref().is_some_and(|a| a.suggestion.keep_name);
                d.form.filename = form::auto_filename(cfg, &d.form, keep, &analysis::original_stem(&d.path));
            }
        }
    }
}

/// Takes a finished analysis into the form.
fn set_form(d: &mut DocItem, analysis: Box<Analysis>, cfg: &Config) {
    d.form = Form::from_analysis(&analysis);
    d.form.filename =
        form::auto_filename(cfg, &d.form, analysis.suggestion.keep_name, &analysis::original_stem(&d.path));
    d.page = 0;
    d.analysis = Some(analysis);
    d.status = DocStatus::Ready;
}

/// Suggestion and filed values in the decision log; on an error, the addition for the notice.
fn log_filed(cfg: &Config, doc: &DocItem, target: &Path) -> String {
    // After a failed analysis (*File anyway*) there was no suggestion.
    let Some(a) = doc.analysis.as_deref().filter(|a| a.suggestion.source != "manual") else { return String::new() };
    if !cfg.log_decisions {
        return String::new();
    }
    let entry = journal::Entry::Filed(Box::new(journal::Filed::new(cfg, a, &doc.form, &doc.path, target)));
    journal_note(journal::append(&journal::path(), &entry))
}

/// Records an undone filing in the log; it then no longer counts in the report.
fn log_undo(cfg: &Config, target: &Path) -> String {
    if !cfg.log_decisions {
        return String::new();
    }
    journal_note(journal::append(&journal::path(), &journal::Entry::Undone(journal::Undone::new(target))))
}

fn journal_note(result: anyhow::Result<()>) -> String {
    result.err().map(|e| (window::t().journal_failed)(&format!("{e:#}"))).unwrap_or_default()
}

/// Page images as textures for the preview.
fn page_textures(ctx: &egui::Context, id: u64, pages: &[analysis::PreviewPage]) -> Vec<TextureHandle> {
    pages.iter().enumerate().map(|(i, p)| page_texture(ctx, id, i, p)).collect()
}

fn page_texture(ctx: &egui::Context, id: u64, page: usize, p: &analysis::PreviewPage) -> TextureHandle {
    let image = ColorImage::from_rgba_unmultiplied(p.size, &p.rgba);
    ctx.load_texture(format!("doc-{id}-{page}"), image, TextureOptions::LINEAR)
}

fn relative(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).display().to_string().replace('\\', "/")
}

/// Rotation per page as the preview shows it: the detected one (analysis) and the manually chosen one combined.
fn page_turns(doc: &DocItem) -> Vec<u16> {
    let detected = doc.analysis.as_ref().map(|a| a.rotations.as_slice()).unwrap_or_default();
    let pages = detected.len().max(doc.turns.len());
    (0..pages).map(|i| (detected.get(i).copied().unwrap_or(0) + doc.turns.get(i).copied().unwrap_or(0)) % 360).collect()
}

/// Rotation per page on filing: the part the file does not contain yet (see `DocItem::baked`).
fn missing_turns(doc: &DocItem) -> Vec<u16> {
    let turns = page_turns(doc);
    let pages = turns.len().max(doc.baked.len());
    (0..pages)
        .map(|i| (turns.get(i).copied().unwrap_or(0) + 360 - doc.baked.get(i).copied().unwrap_or(0) % 360) % 360)
        .collect()
}

/// Properties for Windows Search from the confirmed form and the summary.
fn search_tags(doc: &DocItem) -> tags::Tags {
    form::search_tags(&doc.form, doc.analysis.as_ref().map_or("", |a| a.suggestion.summary.as_str()))
}

fn probe_llm(cfg: &LlmConfig) -> LlmStatus {
    let client = LlmClient::new(cfg);
    match client.list_models() {
        Ok(models) => {
            let model = if cfg.model.trim().is_empty() {
                models.iter().find(|m| !m.to_lowercase().contains("embed")).cloned()
            } else {
                Some(cfg.model.trim().to_string())
            };
            let t = window::t();
            match model {
                Some(model) => {
                    let detail = (t.models_available)(&cfg.base_url, models.len());
                    LlmStatus::Online { model, detail }
                }
                None => LlmStatus::Offline(t.no_model_loaded.into()),
            }
        }
        Err(e) => LlmStatus::Offline(format!("{e:#}")),
    }
}

fn reveal_in_file_manager(path: &Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(path.parent().unwrap_or(path)).spawn();
}

fn pick_files() -> Vec<PathBuf> {
    let readable = ingest::readable_extensions();
    let t = window::t();
    rfd::FileDialog::new()
        .set_title(t.pick_files_title)
        .add_filter(t.documents, &readable)
        .add_filter(t.all_files, &["*"])
        .pick_files()
        .unwrap_or_default()
}

fn pick_folder() -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(window::t().pick_folder_title).pick_folder()
}

impl eframe::App for ScriniumApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::sync(ctx);
        self.handle_events(ctx);
        self.pump_queue();
        // Closing the window with unsaved settings: ask first.
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && let Some(draft) = self.settings.as_mut()
            && draft.request_leave(Leave::Quit).is_none()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }

        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty() {
            self.add_files(dropped);
        }
        // Esc goes back one level per press: first a standing error notice (R6, R7).
        if self.toast.as_ref().is_some_and(|t| t.error)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
        {
            self.toast = None;
        }
        self.shortcuts(ctx);

        self.request_preview();
        self.status_bar(ctx);
        // The settings replace queue and document until they are saved or discarded.
        if let Some(draft) = &mut self.settings {
            self.local_ai.ensure_gpu_checked(ctx);
            let t = window::t();
            let ai = match &self.llm_status {
                LlmStatus::Online { model, detail } => settings::AiStatus {
                    ready: true,
                    text: format!("{model} – {}", detail.replace(crate::i18n::ai::t().embedded_on, "")),
                },
                LlmStatus::Checking => settings::AiStatus { ready: false, text: t.ai_status_checking.into() },
                LlmStatus::Offline(e) => settings::AiStatus { ready: false, text: e.clone() },
                LlmStatus::Disabled => settings::AiStatus { ready: false, text: t.ai_status_off.into() },
            };
            if let Some(action) = settings::view(
                ctx,
                draft,
                &mut self.local_ai,
                &mut self.setup,
                &self.folders,
                &self.config_path,
                settings::AppState { ai: &ai, mode: self.mode, updates: &self.updates },
            ) {
                self.apply_settings(ctx, action);
            }
        } else {
            if !self.docs.is_empty() && !self.queue_hidden {
                self.queue_panel(ctx);
            }
            egui::CentralPanel::default().frame(theme::canvas_frame(&ctx.style())).show(ctx, |ui| {
                let area = ui.max_rect();
                if self.docs.is_empty() {
                    self.empty_state(ui);
                } else {
                    self.detail_view(ui);
                }
                self.settings_corner(ui, area);
            });
        }
        self.notice(ctx);
        self.progress_pills(ctx);
        drop_overlay(ctx);
        self.scan_ui(ctx);
        self.updates_ui(ctx);

        if self.docs.iter().any(|d| matches!(d.status, DocStatus::Queued | DocStatus::Working(_))) {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

impl ScriniumApp {
    /// Ctrl+Enter files, Page Up/Down turns pages, Ctrl+Page Up/Down switches the document.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.settings.is_some() {
            return;
        }
        let typing = ctx.memory(|m| m.focused().is_some());
        if self.shown != self.selected {
            self.shown = self.selected;
            self.shown_since = Instant::now();
        }
        let (later, undo) = ctx.input(|i| {
            let ctrl_shift = i.modifiers.command && i.modifiers.shift;
            let later = ctrl_shift
                && i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, repeat: false, .. }));
            (later, i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::Z))
        });
        if undo && !typing {
            self.undo_last();
        }
        let (confirm, prev_page, next_page, prev_doc, next_doc) = ctx.input(|i| {
            let ctrl = i.modifiers.command && !i.modifiers.shift;
            // Only a real key press counts, no auto-repeat while held.
            let enter = i
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, repeat: false, .. }));
            (
                ctrl && enter,
                !ctrl && i.key_pressed(Key::PageUp),
                !ctrl && i.key_pressed(Key::PageDown),
                ctrl && i.key_pressed(Key::PageUp),
                ctrl && i.key_pressed(Key::PageDown),
            )
        });
        if prev_doc {
            self.select_relative(-1);
        }
        if next_doc {
            self.select_relative(1);
        }
        let Some(id) = self.selected else { return };
        if later && self.shown_since.elapsed() >= CONFIRM_PAUSE {
            self.select_next_after(id);
            return;
        }
        if confirm && self.shown_since.elapsed() >= CONFIRM_PAUSE {
            let cfg = &self.config;
            let ready = self.docs.iter().any(|d| {
                d.id == id && matches!(d.status, DocStatus::Ready) && form::file_blocker(cfg, &d.form).is_none()
            });
            if ready {
                self.confirm(id);
            }
        }
        if !typing && let Some(d) = self.docs.iter_mut().find(|d| d.id == id) {
            if prev_page && d.page > 0 {
                d.page -= 1;
            }
            if next_page && d.page + 1 < d.pages.count() {
                d.page += 1;
            }
        }
    }

    /// Gear at the top right of the main window: opens the settings. A dot signals a new version; the gear then
    /// leads to the *General* tab. The views leave the corner free.
    fn settings_corner(&mut self, ui: &mut egui::Ui, area: egui::Rect) {
        let size = Vec2::splat(theme::ARROW_BUTTON);
        let rect = egui::Rect::from_min_size(area.right_top() - egui::vec2(size.x, 0.0), size);
        let resp = ui.interact(rect, Id::new("settings-corner"), egui::Sense::CLICK);
        theme::paint_icon_button(ui, rect, &resp, icon::SETTINGS, 20.0, true);
        let newer = self.updates.newer();
        if newer.is_some() {
            ui.painter().circle_filled(rect.right_top() + egui::vec2(-6.0, 6.0), 3.5, accent());
        }
        let t = window::t();
        let hover = match newer {
            Some(v) => (t.settings_newer)(&v.to_string()),
            None => t.settings.to_string(),
        };
        if resp.on_hover_text(hover).clicked() {
            self.open_settings(Tab::General);
        }
    }

    /// Progress as a pill with a bar (R6): preparation of a scan and download of a model.
    fn progress_pills(&self, ctx: &egui::Context) {
        let mut slot = 0;
        if self.scan_job.is_some() {
            theme::progress_pill(ctx, slot, &self.scan_step, self.scan_share);
            slot += 1;
        }
        if let Some((text, share)) = self.local_ai.download_status() {
            theme::progress_pill(ctx, slot, &text, share.unwrap_or(0.0));
        }
    }

    /// Notice at the top center above the content (R6): a notice stays 5 s, for long texts 60 ms longer per
    /// character, at most 10 s, and then fades out in 400 ms. An error stays until Esc or a click closes it.
    /// A click closes any notice.
    fn notice(&mut self, ctx: &egui::Context) {
        let Some(toast) = &self.toast else { return };
        let shown = toast.since.elapsed().as_secs_f32();
        let stay = (5.0 + 0.06 * toast.text.chars().count().saturating_sub(40) as f32).min(10.0);
        let opacity = if toast.error { 1.0 } else { (1.0 - (shown - stay) / 0.4).clamp(0.0, 1.0) };
        if opacity <= 0.0 {
            self.toast = None;
            return;
        }
        if !toast.error {
            let next = if shown < stay { stay - shown } else { 0.0 };
            ctx.request_repaint_after(Duration::from_secs_f32(next.max(1.0 / 60.0)));
        }
        let (fill, stroke, color, glyph) = match toast.error {
            true => (tokens::status_err_bg(), status_err(), tokens::fg(), icon::ERROR),
            false => (tokens::surface_raised(), tokens::line(), tokens::fg(), icon::OK),
        };
        let glyph_color = if toast.error { status_err() } else { status_ok() };
        let max_w = (ctx.screen_rect().width() - 160.0).clamp(240.0, 640.0);
        let resp = egui::Area::new(Id::new("notice"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 16.0))
            .show(ctx, |ui| {
                ui.multiply_opacity(opacity);
                egui::Frame::new()
                    .fill(fill)
                    .stroke(Stroke::new(1.0_f32, stroke))
                    .corner_radius(theme::radius::MD)
                    .shadow(tokens::shadow())
                    .inner_margin(Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_max_width(max_w);
                        ui.horizontal(|ui| {
                            ui.label(theme::icon_text(glyph, 16.0, glyph_color));
                            ui.add(egui::Label::new(RichText::new(&toast.text).color(color)).wrap().selectable(false));
                        });
                    })
                    .response
            })
            .inner;
        let close = window::t().notice_close;
        if resp.interact(egui::Sense::CLICK).on_hover_text(close).clicked() {
            self.toast = None;
        }
    }

    fn llm_badge(&mut self, ui: &mut egui::Ui) {
        let embedded = self.config.llm.backend == LlmBackend::Embedded;
        let t = window::t();
        let (color, text, hover) = match (&self.llm_status, self.local_ai.download_status()) {
            (_, Some((progress, fraction))) => {
                (status_warn(), (t.ai_download)(fraction.unwrap_or(0.0) * 100.0), progress)
            }
            (LlmStatus::Disabled, _) => (muted(), t.ai_off.to_string(), t.ai_off_hint.to_string()),
            (LlmStatus::Checking, _) => (status_warn(), t.ai_checking.to_string(), String::new()),
            (LlmStatus::Online { model, detail }, _) => (status_ok(), (t.ai_model)(model), detail.clone()),
            (LlmStatus::Offline(e), _) if embedded => (status_warn(), t.ai_no_model.to_string(), (t.ai_meanwhile)(e)),
            (LlmStatus::Offline(e), _) => (status_err(), t.ai_unreachable.to_string(), (t.ai_meanwhile)(e)),
        };
        let offline = matches!(self.llm_status, LlmStatus::Offline(_));
        let resp = ui.add(egui::Button::new(RichText::new(text).color(color)).frame(false));
        theme::status_dot(ui, color, true);
        let action = if offline { t.ai_click_settings } else { t.ai_click_check };
        if resp.on_hover_text(format!("{hover}\n{action}")).clicked() {
            if offline {
                self.open_settings(Tab::Ai);
            } else {
                let ctx = ui.ctx().clone();
                self.check_llm(&ctx);
            }
        }
    }

    /// Status bar, the only bar: on the left, show or hide the list (in the settings *Back to documents*) and the
    /// counters; on the right, the last filed document with *Undo*, the AI status and the archive. Messages do not
    /// go here (they would move the buttons) but appear as a notice above the content. The gear at the top right
    /// opens the settings.
    fn status_bar(&mut self, ctx: &egui::Context) {
        let mut undo = false;
        let t = window::t();
        egui::TopBottomPanel::bottom("status").frame(theme::status_frame(&ctx.style())).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.settings.is_some() {
                    let back = theme::with_icon(icon::CHEVRON_LEFT, t.back_to_documents);
                    if theme::ghost_button(ui, &back).on_hover_text("Esc").clicked() {
                        let ctx = ui.ctx().clone();
                        self.leave_settings(&ctx);
                    }
                    ui.separator();
                } else if !self.docs.is_empty() {
                    let (glyph, hover) = match self.queue_hidden {
                        true => (icon::PANEL_OPEN, t.show_list),
                        false => (icon::PANEL_CLOSE, t.hide_list),
                    };
                    if theme::icon_button(ui, glyph, true, hover) {
                        self.queue_hidden = !self.queue_hidden;
                        let dir = self.config_path.parent().unwrap_or(Path::new("."));
                        let mut view = theme::read_view(dir);
                        view.list = Some(!self.queue_hidden);
                        theme::write_view(dir, &view);
                    }
                    ui.separator();
                }
                let running = self.docs.iter().filter(|d| d.is_running()).count();
                let waiting = self.docs.iter().filter(|d| d.is_waiting()).count();
                let open = self.docs.iter().filter(|d| matches!(d.status, DocStatus::Ready)).count();
                let filed = self.docs.iter().filter(|d| matches!(d.status, DocStatus::Filed(_))).count();
                let counts = (t.status_counts)(open, running, waiting, self.queue_paused, filed);
                ui.label(RichText::new(counts).color(muted()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let root = self.config.archive_root();
                    let ok = root.is_dir();
                    let label = if ok {
                        (t.archive_status)(&short_root(&root), self.folders.all.len())
                    } else {
                        (t.archive_missing)(&root.display().to_string())
                    };
                    let resp = ui.add(
                        egui::Label::new(RichText::new(label).color(if ok { muted() } else { status_err() }))
                            .sense(egui::Sense::CLICK),
                    );
                    let resp = resp.on_hover_text((t.click_opens_folder)(&root.display().to_string()));
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        if ok {
                            settings::open_path(&root);
                        } else {
                            self.open_settings(Tab::Archive);
                        }
                    }
                    ui.separator();
                    self.llm_badge(ui);
                    if let Some(last) = &self.last_filed {
                        ui.separator();
                        let hover = if last.filed.copied { t.undo_copy_hint_key } else { t.undo_move_hint_key };
                        if theme::ghost_button(ui, &theme::with_icon(icon::UNDO, t.undo)).on_hover_text(hover).clicked()
                        {
                            undo = true;
                        }
                        ui.add(
                            egui::Label::new(
                                RichText::new((t.last_filed)(&relative(&root, &last.filed.target))).color(muted()),
                            )
                            .truncate(),
                        );
                    }
                });
            });
        });
        if undo {
            self.undo_last();
        }
    }

    fn queue_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("queue")
            .resizable(true)
            .default_width(320.0)
            .width_range(220.0..=420.0)
            .frame(theme::bar_frame(&ctx.style()))
            .show(ctx, |ui| {
                // Add more documents: above the list, where they land. Three buttons of equal width; if the words
                // with icon do not fit (longer languages, narrow list), the icons are dropped, and only then does
                // the last button wrap to the next row.
                let t = window::t();
                let (mut pick, mut pick_dir, mut scan_now) = (false, false, false);
                let scanning = self.scan_rx.is_some() || self.scan_job.is_some();
                let scan_label = match (self.scan_rx.is_some(), self.scan_job.is_some()) {
                    (true, _) => t.scanning,
                    (_, true) => t.preparing,
                    _ => t.scan,
                };
                ui.horizontal_wrapped(|ui| {
                    // Tight padding so that icon and word fit into the three buttons of equal width.
                    ui.spacing_mut().button_padding.x = theme::space::S2;
                    let gap = ui.spacing().item_spacing.x;
                    // 1 px slack, otherwise rounding wraps the row.
                    let w = ((ui.available_width() - 2.0 * gap) / 3.0 - 1.0).floor();
                    let size = Vec2::new(w, 0.0);
                    let parts = [
                        (icon::FILE_PLUS, t.files_button),
                        (icon::FOLDER_PLUS, t.folder_button),
                        (icon::SCAN, scan_label),
                    ];
                    let padding = 2.0 * ui.spacing().button_padding.x;
                    let fits = parts.iter().all(|(glyph, label)| {
                        form::text_width(ui, RichText::new(theme::with_icon(*glyph, label)).size(text::BODY)) + padding
                            <= w
                    });
                    let label = |(glyph, label): (char, &str)| match fits {
                        true => theme::with_icon(glyph, label),
                        false => label.to_string(),
                    };
                    pick = ui
                        .add(theme::secondary_button(label(parts[0])).min_size(size))
                        .on_hover_text(t.files_hint)
                        .clicked();
                    pick_dir = ui
                        .add(theme::secondary_button(label(parts[1])).min_size(size))
                        .on_hover_text(t.folder_hint)
                        .clicked();
                    scan_now = ui
                        .add_enabled(!scanning, theme::secondary_button(label(parts[2])).min_size(size))
                        .on_hover_text(t.scan_hint)
                        .on_disabled_hover_text(t.scan_busy)
                        .clicked();
                });
                if pick {
                    let files = pick_files();
                    self.add_files(files);
                }
                if pick_dir && let Some(dir) = pick_folder() {
                    self.add_files(vec![dir]);
                }
                if scan_now {
                    self.start_scan(ctx);
                }
                ui.add_space(4.0);
                // As wide as each of the three buttons above, so right-aligned exactly below the *Scan* button.
                let gap = ui.spacing().item_spacing.x;
                let width = ((ui.available_width() - 2.0 * gap) / 3.0 - 1.0).floor();
                self.scan_as_row(ui, width);
                ui.add_space(12.0);
                let running = self.docs.iter().filter(|d| d.is_running()).count();
                let waiting = self.docs.iter().filter(|d| d.is_waiting()).count();
                let open = self.docs.iter().filter(|d| matches!(d.status, DocStatus::Ready)).count();
                ui.horizontal(|ui| {
                    ui.label(theme::field_label(t.documents));
                    ui.label(RichText::new((t.queue_counts)(open, running + waiting)).small().color(muted()));
                });
                ui.horizontal(|ui| {
                    let pause_label = match self.queue_paused {
                        true => theme::with_icon(icon::PLAY, t.resume),
                        false => theme::with_icon(icon::PAUSE, t.pause),
                    };
                    let can_pause = self.queue_paused || running + waiting > 0;
                    let resp = ui.add_enabled_ui(can_pause, |ui| theme::ghost_button(ui, &pause_label)).inner;
                    if resp.on_hover_text(t.pause_hint).on_disabled_hover_text(t.queue_empty).clicked() {
                        self.queue_paused = !self.queue_paused;
                    }
                    // R1/R2: both buttons are always there and disabled while there is nothing to do.
                    let filed = self.docs.iter().any(|d| matches!(d.status, DocStatus::Filed(_)));
                    let hide = ui.add_enabled_ui(filed, |ui| theme::ghost_button(ui, t.hide_done)).inner;
                    if hide.on_hover_text(t.hide_done_hint).on_disabled_hover_text(t.nothing_filed).clicked() {
                        self.docs.retain(|d| !matches!(d.status, DocStatus::Filed(_)));
                        if !self.docs.iter().any(|d| Some(d.id) == self.selected) {
                            self.selected = self.docs.first().map(|d| d.id);
                        }
                    }
                    let remove = ui
                        .add_enabled_ui(waiting > 0, |ui| {
                            theme::ghost_button(ui, &theme::with_icon(icon::LIST_X, t.remove_waiting))
                        })
                        .inner;
                    if remove.on_disabled_hover_text(t.nothing_waiting).clicked() {
                        self.docs.retain(|d| !d.is_waiting());
                        if !self.docs.iter().any(|d| Some(d.id) == self.selected) {
                            self.selected = self.docs.first().map(|d| d.id);
                        }
                    }
                });
                if self.queue_paused {
                    ui.label(RichText::new(t.paused_note).small().color(status_warn()));
                }
                ui.separator();
                let mut move_id: Option<(u64, isize)> = None;
                let mut clicked = None;
                let mut remove_id = None;
                let mut redo_id = None;
                let cfg = &self.config;
                let folders = &self.folders;
                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for doc in &self.docs {
                        let selected = self.selected == Some(doc.id);
                        let waiting_here = doc.is_waiting();
                        let redo = matches!(doc.status, DocStatus::Ready | DocStatus::Failed(_));
                        let (filled, color, detail) = match &doc.status {
                            DocStatus::Queued if waiting_here => (false, muted(), t.row_waiting.to_string()),
                            DocStatus::Queued => (false, accent(), t.row_next.to_string()),
                            DocStatus::Working(m) => (false, accent(), m.clone()),
                            DocStatus::Ready => match attention(doc, cfg, folders) {
                                Some(why) => (true, status_warn(), (t.row_check)(why, &short_folder(&doc.form.folder))),
                                None => (true, accent(), format!("→ {}", short_folder(&doc.form.folder))),
                            },
                            DocStatus::Filed(f) => (true, status_ok(), (t.row_filed)(&file_name(&f.target))),
                            DocStatus::Failed(e) => (true, status_err(), e.clone()),
                        };
                        ui.horizontal(|ui| {
                            let behind = ui.painter().add(egui::Shape::Noop);
                            let frame = egui::Frame::new().inner_margin(Margin::symmetric(8, 6));
                            let resp = frame
                                .show(ui, |ui| {
                                    // Subtract the room for the cross, the two arrows or *Analyze again* with their
                                    // gaps, and the frame's padding (2 × 8). Otherwise every row sticks out of the
                                    // list, the list grows beyond its maximum width, and a black strip remains
                                    // next to it.
                                    let gap = ui.spacing().item_spacing.x;
                                    let count = 1.0 + if waiting_here { 2.0 } else { f32::from(u8::from(redo)) };
                                    let buttons = count * (theme::ARROW_BUTTON + gap);
                                    ui.set_width((ui.available_width() - buttons - 16.0).max(40.0));
                                    ui.horizontal(|ui| {
                                        if matches!(doc.status, DocStatus::Working(_)) {
                                            ui.add(egui::Spinner::new().size(14.0));
                                        } else {
                                            theme::status_dot(ui, color, filled);
                                        }
                                        ui.vertical(|ui| {
                                            ui.add(
                                                egui::Label::new(RichText::new(&doc.name).family(theme::semibold()))
                                                    .truncate(),
                                            );
                                            ui.add(
                                                egui::Label::new(RichText::new(detail).small().color(muted()))
                                                    .truncate(),
                                            );
                                        });
                                    });
                                })
                                .response;
                            let resp = ui.interact(resp.rect, Id::new(("doc", doc.id)), egui::Sense::CLICK);
                            if resp.clicked() {
                                clicked = Some(doc.id);
                            }
                            // Selected: `highlight` with an accent border (the fill alone barely stands out in dark
                            // mode); under the pointer neutral – blue means "selected".
                            let r = theme::radius::MD;
                            let shape = match (selected, resp.hovered()) {
                                (true, _) => egui::epaint::RectShape::new(
                                    resp.rect,
                                    r,
                                    tokens::highlight(),
                                    Stroke::new(1.0_f32, accent()),
                                    egui::StrokeKind::Inside,
                                )
                                .into(),
                                (false, true) => egui::Shape::rect_filled(resp.rect, r, tokens::hover()),
                                (false, false) => egui::Shape::Noop,
                            };
                            ui.painter().set(behind, shape);
                            if waiting_here {
                                if theme::arrow_button(ui, Arrow::Up, true, t.earlier) {
                                    move_id = Some((doc.id, -1));
                                }
                                if theme::arrow_button(ui, Arrow::Down, true, t.later) {
                                    move_id = Some((doc.id, 1));
                                }
                            }
                            if redo && theme::icon_button(ui, icon::REFRESH, true, t.reanalyze) {
                                redo_id = Some(doc.id);
                            }
                            if theme::icon_button(ui, icon::X, true, t.remove_hint) {
                                remove_id = Some(doc.id);
                            }
                        });
                    }
                });
                if let Some(id) = clicked {
                    self.selected = Some(id);
                }
                // Discard edited suggestions only after confirmation; the question refers to the shown document.
                for (id, action) in [(remove_id, Action::Remove), (redo_id, Action::Reanalyze)]
                    .into_iter()
                    .filter_map(|(id, a)| Some((id?, a)))
                {
                    if self.docs.iter().any(|d| d.id == id && form_edited(d, &self.config)) {
                        self.selected = Some(id);
                        self.pending = Some((id, action));
                    } else if action == Action::Remove {
                        self.remove(id);
                    } else {
                        self.reanalyze(id);
                    }
                }
                if let Some((id, delta)) = move_id {
                    self.move_in_queue(id, delta);
                }
            });
    }

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        let avail = ui.available_size();
        let mut pick = false;
        let mut pick_dir = false;
        let mut scan_now = false;
        let t = window::t();
        ui.vertical_centered(|ui| {
            // At least enough space for the gear at the top right to stay clear.
            ui.add_space((avail.y * 0.12).max(theme::ARROW_BUTTON + 8.0));
            let width = avail.x.min(640.0);
            theme::card_frame().show(ui, |ui| {
                ui.set_width(width - 48.0);
                ui.vertical_centered(|ui| {
                    drop_mark(ui);
                    ui.add_space(8.0);
                    ui.label(RichText::new(t.drop_here).size(text::TITLE).family(theme::semibold()));
                    ui.add_space(8.0);
                    ui.label(RichText::new(t.supported).color(muted()));
                    ui.add_space(12.0);
                    for (n, step) in t.steps.iter().enumerate() {
                        ui.label(RichText::new(format!("{}. {step}", n + 1)).color(muted()));
                    }
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        let size = Vec2::new(170.0, theme::CONTROL);
                        let w = 3.0 * size.x + 2.0 * ui.spacing().item_spacing.x;
                        ui.add_space((ui.available_width() - w).max(0.0) / 2.0);
                        if ui.add(theme::primary_button(t.choose_files).min_size(size)).clicked() {
                            pick = true;
                        }
                        if ui.add(theme::secondary_button(t.choose_folder).min_size(size)).clicked() {
                            pick_dir = true;
                        }
                        let scan_button = theme::secondary_button(theme::with_icon(icon::SCAN, t.scan_dots));
                        if ui
                            .add_enabled(self.scan_rx.is_none(), scan_button.min_size(size))
                            .on_hover_text(t.scan_hint)
                            .on_disabled_hover_text(t.scan_busy)
                            .clicked()
                        {
                            scan_now = true;
                        }
                    });
                    // "Scan as" right-aligned below the *Scan* button, as wide as the button row above.
                    ui.horizontal(|ui| {
                        let size = Vec2::new(170.0, theme::CONTROL);
                        let gap = ui.spacing().item_spacing.x;
                        let w = 3.0 * size.x + 2.0 * gap;
                        ui.add_space((ui.available_width() - w).max(0.0) / 2.0);
                        ui.allocate_ui(Vec2::new(w, theme::CONTROL), |ui| self.scan_as_row(ui, size.x));
                    });
                });
            });
            ui.add_space(16.0);
            let root = self.config.archive_root();
            if !root.is_dir() {
                ui.label(RichText::new((t.archive_absent)(&root.display().to_string())).color(status_warn()));
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 200.0).max(0.0) / 2.0);
                    if ui.add(theme::secondary_button(t.choose_archive)).clicked() {
                        self.open_settings(Tab::Archive);
                    }
                    if ui
                        .add(theme::secondary_button(theme::with_icon(icon::FOLDER_PLUS, t.create_here)))
                        .on_hover_text(root.display().to_string())
                        .clicked()
                    {
                        match std::fs::create_dir_all(&root) {
                            Ok(()) => self.rescan_folders(),
                            Err(e) => self.notify((t.create_failed)(&e.to_string()), true),
                        }
                    }
                });
            } else {
                let summary = (t.archive_summary)(
                    &root.display().to_string(),
                    self.folders.all.len(),
                    self.folders.offered.len(),
                    self.config.rules.iter().filter(|r| r.enabled).count(),
                );
                ui.label(RichText::new(summary).color(muted()));
            }
            match &self.llm_status {
                LlmStatus::Offline(_) => {
                    let msg = if self.config.llm.backend == LlmBackend::Embedded {
                        t.no_model_yet
                    } else {
                        t.server_unreachable
                    };
                    ui.label(RichText::new(msg).color(status_warn()));
                    if ui.add(theme::secondary_button(t.set_up_ai)).clicked() {
                        self.open_settings(Tab::Ai);
                    }
                }
                LlmStatus::Disabled => {
                    ui.label(RichText::new(t.ai_disabled).color(muted()));
                }
                _ => {}
            }
            ui.add_space(12.0);
            if theme::ghost_button(ui, &theme::with_icon(icon::SPARKLES, t.new_here)).clicked() {
                self.open_settings(Tab::Setup);
            }
        });
        if pick {
            let files = pick_files();
            self.add_files(files);
        }
        if pick_dir && let Some(dir) = pick_folder() {
            self.add_files(vec![dir]);
        }
        if scan_now {
            let ctx = ui.ctx().clone();
            self.start_scan(&ctx);
        }
    }

    fn detail_view(&mut self, ui: &mut egui::Ui) {
        let t = window::t();
        let Some(id) = self.selected else {
            ui.centered_and_justified(|ui| ui.label(RichText::new(t.select_document).color(muted())));
            return;
        };
        let Some(idx) = self.docs.iter().position(|d| d.id == id) else { return };

        let mut action: Option<Action> = None;
        let queue_slot = self.queue_slot(id);
        let paused = self.queue_paused;
        let root = self.config.archive_root();
        let (cfg, folders, filter) = (&self.config, &self.folders, &mut self.folder_filter);
        let doc = &mut self.docs[idx];
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // The gear for the settings sits on the right.
                ui.add_space(theme::ARROW_BUTTON + 8.0);
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add(egui::Label::new(RichText::new(&doc.name).heading().family(theme::semibold())).truncate());
                });
            });
        });
        let source_dir = doc.path.parent().map(|p| p.display().to_string()).unwrap_or_default();
        ui.add(egui::Label::new(RichText::new((t.from_dir)(&source_dir)).small().color(muted())).truncate());
        ui.add_space(8.0);

        match &doc.status {
            DocStatus::Queued | DocStatus::Working(_) => {
                let waiting = doc.is_waiting();
                let msg = match &doc.status {
                    DocStatus::Working(m) => m.clone(),
                    DocStatus::Queued if waiting => match queue_slot {
                        Some((i, n)) => (t.waiting_slot)(i, n),
                        None => t.waiting_in_queue.into(),
                    },
                    _ => t.analysed_next.into(),
                };
                let elapsed = doc.started.filter(|_| !waiting).map(|t| t.elapsed().as_secs());
                let status = |ui: &mut egui::Ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() * 0.3);
                        if !waiting {
                            ui.add(egui::Spinner::new().size(36.0).color(accent()));
                            ui.add_space(12.0);
                        }
                        ui.label(RichText::new(&msg).size(text::LARGE));
                        if waiting && paused {
                            ui.label(RichText::new(t.queue_paused).color(status_warn()));
                        } else if waiting {
                            ui.label(RichText::new(t.one_by_one).color(muted()));
                        } else if let Some((i, n)) = queue_slot {
                            ui.label(RichText::new((t.working_slot)(i, n, elapsed)).color(muted()));
                        }
                    });
                };
                let avail = ui.available_size();
                if doc.pages.is_empty() || avail.x <= 760.0 {
                    status(ui);
                } else {
                    // Show the pages already; on the right, where the suggestion will appear, the analysis status.
                    let form_width = (avail.x * 0.46).clamp(340.0, 600.0);
                    let column = Layout::top_down(Align::Min);
                    ui.horizontal_top(|ui| {
                        let gap = 12.0 + 2.0 * ui.spacing().item_spacing.x;
                        let preview_size = Vec2::new(avail.x - form_width - gap, avail.y);
                        ui.allocate_ui_with_layout(preview_size, column, |ui| {
                            ui.set_max_width(preview_size.x);
                            let view = form::Preview {
                                pages: &doc.pages,
                                page: &mut doc.page,
                                zoom: &mut doc.zoom,
                                turns: &mut doc.turns,
                                path: &doc.path,
                                analysis: None,
                            };
                            form::preview_ui(ui, view, preview_size)
                        });
                        ui.add_space(12.0);
                        let form_width = ui.available_width();
                        ui.allocate_ui_with_layout(Vec2::new(form_width, avail.y), column, |ui| {
                            ui.set_width(form_width);
                            ui.set_height(avail.y);
                            status(ui);
                        });
                    });
                }
            }
            DocStatus::Failed(err) => {
                card(ui, status_err(), |ui| {
                    ui.label(
                        RichText::new(theme::with_icon(icon::ERROR, t.analysis_failed))
                            .family(theme::semibold())
                            .color(status_err()),
                    );
                    ui.label(err);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add(theme::primary_button(theme::with_icon(icon::REFRESH, t.try_again))).clicked() {
                            action = Some(Action::Reanalyze);
                        }
                        if ui.add(theme::secondary_button(t.file_anyway)).on_hover_text(t.file_anyway_hint).clicked() {
                            action = Some(Action::Manual);
                        }
                    });
                });
            }
            DocStatus::Filed(filed) => {
                let target = filed.target.clone();
                let copied = filed.copied;
                card(ui, status_ok(), |ui| {
                    ui.label(
                        RichText::new(theme::with_icon(icon::OK, if copied { t.filed_as_copy } else { t.filed }))
                            .family(theme::semibold())
                            .color(status_ok()),
                    );
                    ui.label(RichText::new(relative(&root, &target)).monospace());
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(theme::secondary_button(theme::with_icon(icon::FOLDER_OPEN, t.show_in_file_manager)))
                            .clicked()
                        {
                            reveal_in_file_manager(&target);
                        }
                        if ui
                            .add(theme::secondary_button(theme::with_icon(icon::UNDO, t.undo)))
                            .on_hover_text(t.undo_move_hint)
                            .clicked()
                        {
                            action = Some(Action::Undo);
                        }
                    });
                });
                ui.add_space(12.0);
                let size = ui.available_size();
                let view = form::Preview {
                    pages: &doc.pages,
                    page: &mut doc.page,
                    zoom: &mut doc.zoom,
                    turns: &mut doc.turns,
                    path: &doc.path,
                    analysis: doc.analysis.as_deref(),
                };
                form::preview_ui(ui, view, size);
            }
            DocStatus::Ready => {
                let Some(analysis) = doc.analysis.as_deref() else { return };
                let keep_name = analysis.suggestion.keep_name;
                // Side by side from medium width on; the form gets narrower rather than the preview tiny.
                let wide = ui.available_width() > 760.0;
                let avail = ui.available_size();
                let ctx = FormCtx {
                    cfg,
                    folders,
                    filter,
                    path: &doc.path,
                    keep_name,
                    doc_id: doc.id,
                    error: doc.file_error.as_deref(),
                };
                if wide {
                    let form_width = (avail.x * 0.46).clamp(340.0, 600.0);
                    let column = Layout::top_down(Align::Min);
                    ui.horizontal_top(|ui| {
                        // Gap: 12 px plus twice the spacing between items.
                        let gap = 12.0 + 2.0 * ui.spacing().item_spacing.x;
                        let preview_size = Vec2::new(avail.x - form_width - gap, avail.y);
                        ui.allocate_ui_with_layout(preview_size, column, |ui| {
                            ui.set_max_width(preview_size.x);
                            let view = form::Preview {
                                pages: &doc.pages,
                                page: &mut doc.page,
                                zoom: &mut doc.zoom,
                                turns: &mut doc.turns,
                                path: &doc.path,
                                analysis: Some(analysis),
                            };
                            form::preview_ui(ui, view, preview_size)
                        });
                        ui.add_space(12.0);
                        // The form takes exactly the rest so that nothing sticks out over the edge.
                        let form_width = ui.available_width();
                        ui.allocate_ui_with_layout(Vec2::new(form_width, avail.y), column, |ui| {
                            ui.set_width(form_width);
                            ui.set_height(avail.y);
                            if let Some(a) = form::form_ui(ui, &mut doc.form, analysis, ctx) {
                                action = Some(Action::from(a));
                            }
                        });
                    });
                } else {
                    let form_h = (avail.y * 0.6).max(360.0);
                    ui.allocate_ui_with_layout(Vec2::new(avail.x, form_h), Layout::top_down(Align::Min), |ui| {
                        ui.set_height(form_h);
                        if let Some(a) = form::form_ui(ui, &mut doc.form, analysis, ctx) {
                            action = Some(Action::from(a));
                        }
                    });
                    ui.add_space(8.0);
                    let size = ui.available_size();
                    let view = form::Preview {
                        pages: &doc.pages,
                        page: &mut doc.page,
                        zoom: &mut doc.zoom,
                        turns: &mut doc.turns,
                        path: &doc.path,
                        analysis: Some(analysis),
                    };
                    form::preview_ui(ui, view, size);
                }
            }
        }

        // Remove and analyze again discard edits: ask first.
        if let Some(a @ (Action::Remove | Action::Reanalyze)) = action
            && self.docs.iter().any(|d| d.id == id && form_edited(d, &self.config))
        {
            self.pending = Some((id, a));
            action = None;
        }
        if let Some((pid, pa)) = self.pending
            && let Some(answer) = discard_dialog(ui.ctx(), pa)
        {
            self.pending = None;
            if answer && pid == id {
                action = Some(pa);
            }
        }
        match action {
            Some(Action::Confirm) => self.confirm(id),
            Some(Action::Reanalyze) => self.reanalyze(id),
            Some(Action::Remove) => self.remove(id),
            Some(Action::Skip) => self.select_next_after(id),
            Some(Action::Undo) => self.undo(id),
            Some(Action::Manual) => self.file_manually(id),
            None => {}
        }
    }
}

/// Pause after switching documents before Ctrl+Enter files again.
const CONFIRM_PAUSE: Duration = Duration::from_millis(400);

/// Longer side of the scan preview in the dialog: large enough for black-and-white text to appear as text there.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Action {
    Confirm,
    Reanalyze,
    Remove,
    Skip,
    Undo,
    Manual,
}

impl From<FormAction> for Action {
    fn from(a: FormAction) -> Self {
        match a {
            FormAction::Confirm => Action::Confirm,
            FormAction::Skip => Action::Skip,
        }
    }
}

/// Result of a document (error, filed): status color as a border on its background.
fn card(ui: &mut egui::Ui, color: Color32, add: impl FnOnce(&mut egui::Ui)) {
    let fill = if color == status_err() { tokens::status_err_bg() } else { tokens::status_ok_bg() };
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, color))
        .corner_radius(theme::radius::LG)
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// Does a finished document need special attention? Returns the reason.
fn attention(d: &DocItem, cfg: &Config, folders: &FolderIndex) -> Option<&'static str> {
    let a = d.analysis.as_deref()?;
    let s = &a.suggestion;
    let t = window::t();
    if s.origin.folder == Origin::Fallback {
        return Some(t.no_matching_folder);
    }
    if s.confidence_from_ai && s.confidence < 0.6 {
        return Some(t.uncertain);
    }
    if !cfg.archive_root().is_dir() {
        return Some(t.archive_unreachable);
    }
    if !d.form.folder.trim().is_empty() && !folders.exists(&d.form.folder) {
        return Some(t.new_folder);
    }
    if !a.warnings.is_empty() {
        return Some(t.note);
    }
    None
}

/// Were fields changed compared with the suggestion?
fn form_edited(d: &DocItem, cfg: &Config) -> bool {
    let Some(a) = d.analysis.as_deref() else { return false };
    if !matches!(d.status, DocStatus::Ready) {
        return false;
    }
    let mut fresh = Form::from_analysis(a);
    fresh.filename = form::auto_filename(cfg, &fresh, a.suggestion.keep_name, &analysis::original_stem(&d.path));
    fresh != d.form
}

/// Confirmation before edits are discarded. `Some(true)` = discard, `Some(false)` = back.
fn discard_dialog(ctx: &egui::Context, action: Action) -> Option<bool> {
    let t = window::t();
    let what = if action == Action::Remove { t.remove_from_list } else { t.reanalyze };
    let mut answer = None;
    let enter = theme::dialog_enter(ctx);
    let modal = theme::dialog(Id::new("discard-edits")).show(ctx, |ui| {
        ui.set_width(400.0);
        ui.label(theme::dialog_title(t.discard_title));
        ui.add_space(8.0);
        ui.label((t.discard_text)(what));
        ui.add_space(16.0);
        // Buttons on the right; the edits get lost, hence the danger button on the outside.
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(theme::danger_button(what)).clicked() || enter {
                answer = Some(true);
            }
            if ui.add(theme::secondary_button(t.back)).clicked() {
                answer = Some(false);
            }
        });
    });
    if answer.is_none() && modal.should_close() {
        answer = Some(false);
    }
    answer
}

/// Last two parts of a folder: `…/Gehalt/2026_Max`.
fn short_folder(folder: &str) -> String {
    let parts = archive::split_folder(folder);
    match parts.len() {
        0 => window::t().archive.into(),
        1 | 2 => parts.join("/"),
        n => format!("…/{}", parts[n - 2..].join("/")),
    }
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Icon of the empty state: Lucide "download" at 32 px.
fn drop_mark(ui: &mut egui::Ui) {
    ui.label(theme::icon_text(icon::DOWNLOAD, 32.0, tokens::accent_strong()));
}

fn drop_overlay(ctx: &egui::Context) {
    let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
    if !hovering {
        return;
    }
    // Dimmed as behind a dialog card; the prompt sits on a card with an accent border.
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("drop-overlay")));
    let rect = ctx.screen_rect();
    painter.rect_filled(rect, 0.0, tokens::scrim());
    let font = FontId::new(text::TITLE, theme::semibold());
    let galley = painter.layout_no_wrap(window::t().drop_release.to_string(), font, tokens::fg());
    let card = egui::Rect::from_center_size(rect.center(), galley.size() + Vec2::new(64.0, 48.0));
    painter.add(tokens::shadow().as_shape(card, theme::radius::XL));
    let stroke = Stroke::new(2.0_f32, accent());
    painter.rect(card, theme::radius::XL, tokens::surface_raised(), stroke, egui::StrokeKind::Inside);
    painter.galley(card.center() - galley.size() / 2.0, galley, tokens::fg());
}

/// The last two folders of the archive path for the status bar; a short path like `D:/Ablage` is shown in full.
fn short_root(root: &Path) -> String {
    let names: Vec<String> = root
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(n) => Some(n.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    if names.len() <= 2 {
        return root.display().to_string().replace('\\', "/");
    }
    format!("…/{}", names[names.len() - 2..].join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_root_keeps_short_paths() {
        assert_eq!(short_root(Path::new("D:/Ablage")), "D:/Ablage");
        assert_eq!(short_root(Path::new("/home/max/Ablage/Archiv")), "…/Ablage/Archiv");
        assert_eq!(short_root(Path::new("/srv/Ablage")), "/srv/Ablage");
    }
}
