//! Settings of the embedded AI: check the graphics card, download models, fine-tuning.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, RichText, TextEdit};

use crate::config::LlmConfig;
use crate::download::{self, Progress};
use crate::i18n::ai;
use crate::local_llm::{self, Device, ModelSpec, Recommendation};
use crate::theme::{self, tokens};
use tokens::{accent, muted, status_err, status_ok, status_warn};

pub struct GpuReport {
    pub devices: Vec<Device>,
    pub recommendation: Recommendation,
}

fn gpu_report() -> Result<GpuReport, String> {
    let devices = local_llm::detect_devices().map_err(|e| format!("{e:#}"))?;
    let recommendation = local_llm::recommend(&devices);
    Ok(GpuReport { devices, recommendation })
}

/// Model name and device description for the status display.
pub fn probe(cfg: &LlmConfig) -> Result<(String, String), String> {
    let t = ai::t();
    let devices = local_llm::detect_devices().map_err(|e| format!("{e:#}"))?;
    let files = local_llm::resolve_files(cfg, &devices).map_err(|e| format!("{e:#}"))?;
    let device = match local_llm::best_gpu(&devices).filter(|_| cfg.gpu_layers != 0) {
        Some(g) => g.summary(),
        None => t.cpu_no_gpu.to_string(),
    };
    Ok((files.name, format!("{}{device}", t.embedded_on)))
}

struct DownloadTask {
    spec: &'static ModelSpec,
    progress: Arc<Mutex<Progress>>,
    cancel: Arc<AtomicBool>,
    rx: Receiver<Result<(), String>>,
}

#[derive(Default)]
pub struct LocalAi {
    gpu: Option<Result<GpuReport, String>>,
    gpu_rx: Option<Receiver<Result<GpuReport, String>>>,
    download: Option<DownloadTask>,
    last_error: Option<String>,
    /// Model whose deletion still needs confirmation.
    confirm_delete: Option<&'static ModelSpec>,
}

impl LocalAi {
    pub fn check_gpu(&mut self, ctx: &egui::Context) {
        let (tx, rx) = channel();
        self.gpu_rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(gpu_report());
            ctx.request_repaint();
        });
    }

    pub fn ensure_gpu_checked(&mut self, ctx: &egui::Context) {
        if self.gpu.is_none() && self.gpu_rx.is_none() {
            self.check_gpu(ctx);
        }
    }

    /// Message for the status line once a download has finished (text, error?).
    pub fn poll(&mut self) -> Option<(String, bool)> {
        if let Some(rx) = &self.gpu_rx
            && let Ok(r) = rx.try_recv()
        {
            self.gpu = Some(r);
            self.gpu_rx = None;
        }
        let task = self.download.as_ref()?;
        let result = task.rx.try_recv().ok()?;
        let name = task.spec.name;
        self.download = None;
        let t = ai::t();
        Some(match result {
            Ok(()) => {
                self.last_error = None;
                ((t.model_ready)(name), false)
            }
            Err(e) => {
                self.last_error = Some(e.clone());
                ((t.download_of)(name, &e), true)
            }
        })
    }

    /// Progress of a running download (description, fraction).
    pub fn download_status(&self) -> Option<(String, Option<f32>)> {
        let task = self.download.as_ref()?;
        let p = task.progress.lock().ok()?;
        Some((p.describe(), p.fraction()))
    }

    fn start_download(&mut self, spec: &'static ModelSpec, ctx: &egui::Context) {
        if self.download.is_some() {
            return;
        }
        let progress = Arc::new(Mutex::new(Progress { label: spec.name.to_string(), done: 0, total: None }));
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();
        let (p, c, ctx) = (progress.clone(), cancel.clone(), ctx.clone());
        let started = std::thread::Builder::new().name("model-download".into()).spawn(move || {
            let mut report = |state: &Progress| {
                if let Ok(mut guard) = p.lock() {
                    *guard = state.clone();
                }
                ctx.request_repaint();
            };
            let result = local_llm::download_model(spec, &mut report, &c).map_err(|e| format!("{e:#}"));
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        if let Err(e) = started {
            self.last_error = Some(e.to_string());
            return;
        }
        self.last_error = None;
        self.download = Some(DownloadTask { spec, progress, cancel, rx });
    }

    pub fn settings_ui(&mut self, ui: &mut egui::Ui, cfg: &mut LlmConfig) {
        ui.label(RichText::new(ai::t().runs_locally).small().color(muted()));
        ui.add_space(4.0);
        self.gpu_ui(ui);
        ui.add_space(8.0);
        self.models_ui(ui, cfg);
        ui.add_space(4.0);
        advanced_ui(ui, cfg);
    }

    fn gpu_ui(&mut self, ui: &mut egui::Ui) {
        let t = ai::t();
        ui.horizontal(|ui| {
            ui.label(theme::field_label(t.graphics_card));
            let checking = self.gpu_rx.is_some();
            if ui
                .add_enabled(!checking, theme::secondary_button(theme::with_icon(theme::icon::REFRESH, t.check_again)))
                .on_disabled_hover_text(t.gpu_checking)
                .clicked()
            {
                let ctx = ui.ctx().clone();
                self.check_gpu(&ctx);
            }
            if checking {
                ui.add(egui::Spinner::new());
                ui.label(RichText::new(t.searching_gpus).color(muted()));
            }
        });
        match &self.gpu {
            None => {}
            Some(Err(e)) => {
                ui.label(RichText::new(e).color(status_err()));
            }
            Some(Ok(report)) => {
                for d in &report.devices {
                    let best = report.recommendation.gpu.as_ref().is_some_and(|g| g.index == d.index);
                    ui.horizontal_wrapped(|ui| {
                        let color = if best { status_ok() } else { ui.visuals().text_color() };
                        let mut text = (t.device_line)(d.kind_label(), &d.summary());
                        if d.is_gpu() {
                            text.push_str(&(t.device_free)(&download::format_bytes(d.memory_free)));
                        }
                        ui.label(RichText::new(text).color(color));
                        if best {
                            ui.label(RichText::new(t.in_use).small().color(status_ok()));
                        }
                    });
                }
                if report.recommendation.gpu.is_none() {
                    ui.label(RichText::new(t.no_gpu_hint).small().color(status_warn()));
                }
                ui.label(RichText::new((t.recommendation)(&report.recommendation.reason)).color(muted()));
            }
        }
    }

    fn models_ui(&mut self, ui: &mut egui::Ui, cfg: &mut LlmConfig) {
        let t = ai::t();
        let recommended = self.gpu.as_ref().and_then(|r| r.as_ref().ok()).map(|r| r.recommendation.model);
        ui.add_space(4.0);
        ui.label(theme::field_label(t.model));
        let own_file = !cfg.model_path.trim().is_empty();
        if own_file {
            ui.label(RichText::new(t.own_model_file).small().color(status_warn()));
        }
        let auto_label = match recommended {
            Some(m) => (t.auto_for_hardware)(m.name),
            None => t.auto_matching.to_string(),
        };
        theme::radio_value(ui, &mut cfg.model_id, String::new(), auto_label);

        let mut start: Option<&'static ModelSpec> = None;
        let mut delete: Option<&'static ModelSpec> = None;
        let mut cancel = false;
        egui::Grid::new("g-models").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
            for spec in local_llm::CATALOG {
                theme::radio_value(ui, &mut cfg.model_id, spec.id.to_string(), spec.name);
                ui.label(download::format_bytes(spec.total_size()));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(spec.note.text()).small().color(muted()));
                    if recommended == Some(spec) {
                        theme::chip(ui, t.recommended, accent());
                    }
                });
                ui.horizontal(|ui| match &self.download {
                    Some(task) if task.spec == spec => {
                        let p = task.progress.lock().map(|p| p.clone()).unwrap_or_default();
                        // The bar sits in the progress pill at the bottom (R6); only the status here.
                        ui.label(RichText::new(short_progress(&p)).small().color(muted())).on_hover_text(p.describe());
                        if theme::ghost_button(ui, t.cancel).clicked() {
                            cancel = true;
                        }
                    }
                    _ if spec.is_installed() => {
                        ui.label(RichText::new(t.installed).color(status_ok()));
                        if theme::ghost_button(ui, &theme::with_icon(theme::icon::TRASH, t.delete))
                            .on_hover_text(t.delete_hover)
                            .clicked()
                        {
                            delete = Some(spec);
                        }
                    }
                    _ => {
                        let idle = self.download.is_none();
                        if ui
                            .add_enabled(
                                idle,
                                theme::secondary_button(theme::with_icon(theme::icon::DOWNLOAD, t.download)),
                            )
                            .on_hover_text(spec.repo)
                            .on_disabled_hover_text(t.download_running)
                            .clicked()
                        {
                            start = Some(spec);
                        }
                    }
                });
                ui.end_row();
            }
        });
        if let Some(e) = &self.last_error {
            ui.label(RichText::new(e).small().color(status_err()));
        }
        if cancel && let Some(task) = &self.download {
            task.cancel.store(true, Ordering::Relaxed);
        }
        if let Some(spec) = start {
            let ctx = ui.ctx().clone();
            self.start_download(spec, &ctx);
        }
        if delete.is_some() {
            self.confirm_delete = delete;
        }
        self.delete_dialog(ui.ctx());
        ui.label(
            RichText::new((t.source_and_location)(&local_llm::models_dir().display().to_string()))
                .small()
                .color(muted()),
        );
    }

    /// Confirmation before deleting a model: downloading it again takes long.
    fn delete_dialog(&mut self, ctx: &egui::Context) {
        let Some(spec) = self.confirm_delete else { return };
        let t = ai::t();
        let mut close = false;
        let enter = theme::dialog_enter(ctx);
        let modal = theme::dialog(egui::Id::new("model-delete")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(theme::dialog_title(&(t.delete_title)(spec.name)));
            ui.add_space(8.0);
            ui.label((t.delete_text)(&download::format_bytes(spec.total_size())));
            ui.add_space(16.0);
            // Irreversible: danger button on the far right.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme::danger_button(t.delete).min_size(egui::vec2(120.0, 0.0))).clicked() || enter {
                    if let Err(e) = local_llm::delete_model(spec) {
                        self.last_error = Some(format!("{e:#}"));
                    }
                    close = true;
                }
                if ui.add(theme::secondary_button(t.cancel)).clicked() {
                    close = true;
                }
            });
        });
        if close || modal.should_close() {
            self.confirm_delete = None;
        }
    }
}

fn short_progress(p: &Progress) -> String {
    match p.fraction() {
        Some(f) => format!("{:.0} % · {}", f * 100.0, download::format_bytes(p.done)),
        None => download::format_bytes(p.done),
    }
}

fn advanced_ui(ui: &mut egui::Ui, cfg: &mut LlmConfig) {
    let t = ai::t();
    theme::collapsing(t.advanced).id_salt("llm-advanced").show(ui, |ui| {
        ui.label(RichText::new(t.llama_cpp_hint).small().color(muted()));
        egui::Grid::new("g-llm-adv").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label(t.gpu_layers);
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut cfg.gpu_layers).range(-1..=999));
                let hint = match cfg.gpu_layers {
                    -1 => t.layers_all.to_string(),
                    0 => t.layers_none.to_string(),
                    n => (t.layers_some)(n),
                };
                ui.label(RichText::new(hint).small().color(muted()));
            });
            ui.end_row();
            ui.label(t.context_length);
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut cfg.context_size).range(2048..=32768).speed(256));
                ui.label(RichText::new(t.context_hint).small().color(muted()));
            });
            ui.end_row();
            ui.label(t.max_chars);
            ui.add(egui::DragValue::new(&mut cfg.max_chars).range(500..=30000).speed(100));
            ui.end_row();
            ui.label(t.own_gguf);
            path_field(ui, &mut cfg.model_path, t.own_gguf_hint, t.choose_model);
            ui.end_row();
            ui.label(t.image_projector);
            path_field(ui, &mut cfg.mmproj_path, t.projector_hint, t.choose_projector);
            ui.end_row();
        });
    });
}

fn path_field(ui: &mut egui::Ui, value: &mut String, hint: &str, title: &str) {
    ui.horizontal(|ui| {
        theme::field(ui, TextEdit::singleline(value).hint_text(hint).desired_width(280.0));
        if ui.button("…").clicked()
            && let Some(p) = rfd::FileDialog::new().set_title(title).add_filter("GGUF", &["gguf"]).pick_file()
        {
            *value = p.display().to_string();
        }
    });
}
