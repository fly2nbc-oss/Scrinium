//! Analysis pipeline: load → align → OCR → AI/heuristics → rules → suggestion.
//! Runs in its own thread so the UI stays responsive.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use anyhow::Result;
use base64::Engine as _;
use chrono::{Datelike, Local, NaiveDate};
use image::DynamicImage;
use image::codecs::jpeg::JpegEncoder;

use crate::archive::{self, FolderIndex};
use crate::config::{Config, LlmBackend};
use crate::heuristics::{self, DateRank};
use crate::i18n;
use crate::ingest::{self, DocKind};
use crate::llm::{self, LlmAnalysis, LlmClient, LlmRequest, SchemaSpec};
use crate::local_llm::{LocalLlm, PromptTooLong};
use crate::naming::{self, DocDate};
use crate::ocr::Ocr;
use crate::preprocess;
use crate::setup;

/// Longer side of the preview images, for the pages of the analysis as well as for pages loaded later.
const PREVIEW_MAX_SIDE: u32 = 1400;
const OCR_MAX_SIDE: u32 = 2800;
const VISION_MAX_SIDE: u32 = 1600;
/// At 1280 px side length (A4), Qwen3-VL needs a good 1100 image tokens.
const EMBEDDED_VISION_MAX_SIDE: u32 = 1280;
const MAX_FOLDERS_IN_PROMPT: usize = 500;

/// Where a value in the suggestion comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Origin {
    #[default]
    None,
    /// From the AI.
    Ai,
    /// From a special rule.
    Rule,
    /// From the folder keywords.
    Words,
    /// From the recognised text, without AI.
    Text,
    /// From the file name.
    Filename,
    /// From the target folder (e.g. the person in `Zeugnisse_Max`).
    Folder,
    /// Nothing fit: fallback folder.
    Fallback,
}

impl Origin {
    pub fn label(self) -> &'static str {
        let t = i18n::analysis::t();
        match self {
            Origin::None => "",
            Origin::Ai => t.origin_ai,
            Origin::Rule => t.origin_rule,
            Origin::Words => t.origin_keyword,
            Origin::Text => t.origin_text,
            Origin::Filename => t.origin_filename,
            Origin::Folder => t.origin_folder,
            Origin::Fallback => t.origin_fallback,
        }
    }

    /// Fixed name for the decision log, independent of the language.
    pub fn key(self) -> &'static str {
        match self {
            Origin::None => "",
            Origin::Ai => "ai",
            Origin::Rule => "rule",
            Origin::Words => "words",
            Origin::Text => "text",
            Origin::Filename => "filename",
            Origin::Folder => "folder",
            Origin::Fallback => "fallback",
        }
    }
}

/// Origin of each field of the suggestion.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Origins {
    pub date: Origin,
    pub doctype: Origin,
    pub sender: Origin,
    pub subject: Origin,
    pub person: Origin,
    pub extra: Origin,
    pub folder: Origin,
}

#[derive(Debug, Clone, Default)]
pub struct Suggestion {
    pub date: Option<DocDate>,
    pub doctype: String,
    pub sender: String,
    pub subject: String,
    pub person: String,
    pub extra: String,
    pub summary: String,
    pub folder: String,
    /// Short notes on the folder, e.g. “year folder for 2026”.
    pub folder_notes: Vec<String>,
    /// The AI's reasons for each stage of the folder choice.
    pub folder_why: Vec<String>,
    pub confidence: f32,
    /// Where the suggestion comes from, e.g. "AI (Qwen3-VL 8B, …)" or "heuristics" (compared in `form.rs`).
    pub source: String,
    /// Description of the applied rule.
    pub rule: Option<String>,
    /// Program file: the name stays unchanged.
    pub keep_name: bool,
    /// Origin of each field.
    pub origin: Origins,
    /// The confidence comes from the AI (otherwise it is only a placeholder and is not shown).
    pub confidence_from_ai: bool,
}

pub struct PreviewPage {
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
}

pub struct Analysis {
    pub suggestion: Suggestion,
    pub kind: DocKind,
    pub text: String,
    pub previews: Vec<PreviewPage>,
    pub page_count: usize,
    pub ocr_engine: Option<String>,
    /// Notes on the image correction, e.g. "Page 1: rotated 180°, straightened 3.2°".
    pub corrections: Vec<String>,
    /// Detected coarse rotation per preview page (clockwise, 0/90/180/270). Written into the file on filing,
    /// together with the rotation chosen by hand.
    pub rotations: Vec<u16>,
    pub warnings: Vec<String>,
    pub duration: Duration,
    /// Timing of the embedded AI, e.g. “input 7400 tokens in 6.1 s, answer 180 tokens in 4.0 s”.
    pub ai_stats: Option<String>,
}

pub struct Job {
    pub id: u64,
    pub path: PathBuf,
    pub config: Config,
    pub folders: FolderIndex,
}

/// Job for the background thread: analyse a document or generate the settings from a
/// description. Both use the same loaded model.
pub enum Work {
    Analyse(Job),
    Setup(SetupJob),
}

pub struct SetupJob {
    pub config: Config,
    pub description: String,
    pub folders: FolderIndex,
    /// Set when the job was cancelled.
    pub cancel: Arc<AtomicBool>,
}

pub enum Event {
    Progress {
        id: u64,
        message: String,
    },
    Done {
        id: u64,
        analysis: Box<Analysis>,
    },
    /// Page images for the preview, independent of the analysis (not aligned). `page_count` is the document's
    /// number of pages, 0 if it could not be opened; a page that failed comes with its error.
    Preview {
        id: u64,
        page_count: usize,
        pages: Vec<(usize, Result<PreviewPage, String>)>,
    },
    Failed {
        id: u64,
        error: String,
    },
    SetupProgress(String),
    SetupDone(Result<Box<setup::Proposal>, String>),
    /// The job was cancelled; its result does not count.
    SetupCancelled,
}

/// Preview job: single page images, without analysis.
pub struct PreviewJob {
    pub id: u64,
    pub path: PathBuf,
    /// Counted from 0; only a few at a time (see `pages::PageCache::wanted`).
    pub pages: Vec<usize>,
    /// Rotation per page (clockwise) that the file already contains because it was filed rotated. The preview
    /// turns the pages back by that much, so it shows them as they looked before filing.
    pub turned: Vec<u16>,
}

/// Starts the analysis thread and, next to it, the preview thread, which loads page images while the analysis
/// is still working on another document. `notify` wakes the UI after each event.
pub fn spawn_worker(notify: impl Fn() + Send + Sync + 'static) -> (Sender<Work>, Sender<PreviewJob>, Receiver<Event>) {
    let (job_tx, job_rx) = channel::<Work>();
    let (event_tx, event_rx) = channel::<Event>();
    let (preview_tx, preview_rx) = channel::<PreviewJob>();
    let notify = Arc::new(notify);
    let (preview_events, preview_notify) = (event_tx.clone(), Arc::clone(&notify));
    let started = std::thread::Builder::new().name("preview".into()).spawn(move || {
        for job in preview_rx {
            let event = match ingest::preview_pages(&job.path, job.pages.clone(), PREVIEW_MAX_SIDE) {
                Ok(found) => Event::Preview {
                    id: job.id,
                    page_count: found.page_count,
                    pages: (found.pages.into_iter())
                        .map(|(i, image)| {
                            let back = (360 - job.turned.get(i).copied().unwrap_or(0) % 360) % 360;
                            let upright = |img| if back == 0 { img } else { preprocess::rotate_dynamic(&img, back) };
                            (i, image.map(|img| to_preview(&upright(img))))
                        })
                        .collect(),
                },
                // The pages show the error instead of the document seeming to end there.
                Err(e) => {
                    let error = format!("{e:#}");
                    let pages = job.pages.iter().map(|&i| (i, Err(error.clone()))).collect();
                    Event::Preview { id: job.id, page_count: 0, pages }
                }
            };
            let _ = preview_events.send(event);
            preview_notify();
        }
    });
    if let Err(e) = started {
        eprintln!("Could not start the preview thread: {e}");
    }
    let started = std::thread::Builder::new().name("analysis".into()).spawn(move || {
        let mut worker = Worker::default();
        for work in job_rx {
            let job = match work {
                Work::Analyse(job) => job,
                Work::Setup(setup) => {
                    let progress = |message: &str| {
                        if !setup.cancel.load(Ordering::Relaxed) {
                            let _ = event_tx.send(Event::SetupProgress(message.to_string()));
                            notify();
                        }
                    };
                    let result = worker.setup(&setup, &progress).map(Box::new).map_err(|e| format!("{e:#}"));
                    let event = match setup.cancel.load(Ordering::Relaxed) {
                        true => Event::SetupCancelled,
                        false => Event::SetupDone(result),
                    };
                    let _ = event_tx.send(event);
                    notify();
                    continue;
                }
            };
            let id = job.id;
            let progress = |message: &str| {
                let _ = event_tx.send(Event::Progress { id, message: message.to_string() });
                notify();
            };
            let event = match worker.run(&job, &progress) {
                Ok(analysis) => Event::Done { id, analysis: Box::new(analysis) },
                Err(e) => Event::Failed { id, error: format!("{e:#}") },
            };
            let _ = event_tx.send(event);
            notify();
        }
    });
    if let Err(e) = started {
        eprintln!("Could not start the analysis thread: {e}");
    }
    (job_tx, preview_tx, event_rx)
}

/// Inputs for the AI.
#[derive(Clone, Copy)]
struct Prompt<'a> {
    cfg: &'a Config,
    folders: &'a [String],
    text: &'a str,
    filename: &'a str,
    kind: DocKind,
}

/// The AI's answer for a document.
struct AiAnswer {
    /// Model and compute device, for the origin of the suggestion.
    model: String,
    answer: LlmAnalysis,
    /// Reasons for the folder choice.
    why: Vec<String>,
    /// Keywords decided the folder on every level.
    by_words: bool,
    /// Timings, for display and measuring.
    stats: Option<String>,
}

#[derive(Default)]
struct Worker {
    ocr: Option<Ocr>,
    /// Stays loaded between documents; loading takes a few seconds depending on the model.
    local: Option<LocalLlm>,
}

impl Worker {
    /// The embedded model; loaded on first use and kept loaded between documents.
    fn local(&mut self, cfg: &Config, progress: &dyn Fn(&str), warnings: &mut Vec<String>) -> Result<&mut LocalLlm> {
        let key = LocalLlm::key_for(&cfg.llm)?;
        // A different model is released first, then the new one is loaded (graphics memory).
        match self.local.take() {
            Some(llm) if llm.key() == &key => Ok(self.local.insert(llm)),
            _ => {
                progress(i18n::analysis::t().loading_ai_model);
                Ok(self.local.insert(LocalLlm::load(key, warnings)?))
            }
        }
    }

    /// One request about the document with the recognised text and optionally the page image. `user` builds the
    /// prompt for a text length; if it does not fit into the context, first less text, then no image.
    /// Returns the model's label and the JSON of the answer.
    fn ask_document(
        &mut self,
        p: &Prompt,
        system: &str,
        schema: &serde_json::Value,
        page: Option<&DynamicImage>,
        user: &dyn Fn(usize) -> String,
        warnings: &mut Vec<String>,
    ) -> Result<(String, serde_json::Value)> {
        let cfg = p.cfg;
        let t = i18n::analysis::t();
        match cfg.llm.backend {
            LlmBackend::Embedded => {
                let llm = self.local.as_mut().ok_or_else(|| anyhow::anyhow!(t.ai_model_not_loaded))?;
                let mut image = match (llm.supports_vision(), page) {
                    (true, Some(img)) => Some(limit_size(img, EMBEDDED_VISION_MAX_SIDE)),
                    _ => None,
                };
                let mut chars = cfg.llm.max_chars;
                loop {
                    match llm.generate(system, &user(chars), image.as_ref(), schema) {
                        Ok(raw) => return Ok((format!("{}, {}", llm.name, llm.device), llm::extract_json(&raw)?)),
                        Err(e) if e.downcast_ref::<PromptTooLong>().is_some() && chars > 1500 && !p.text.is_empty() => {
                            chars /= 2;
                        }
                        Err(e) if image.is_some() && !p.text.trim().is_empty() => {
                            warnings.push((t.ai_without_page_image)(&format!("{e:#}")));
                            image = None;
                            chars = cfg.llm.max_chars;
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
            LlmBackend::Server => {
                let client = LlmClient::new(&cfg.llm);
                let model = client.resolve_model(&cfg.llm.model)?;
                let image = page.map(encode_jpeg_b64).transpose()?;
                let user = user(cfg.llm.max_chars);
                let value = match client.complete_json(&model, system, &user, image.as_deref(), schema, 800) {
                    Ok(v) => v,
                    // Text models without image support reject image requests; then retry with text only.
                    Err(e) if image.is_some() && !p.text.trim().is_empty() => client
                        .complete_json(&model, system, &user, None, schema, 800)
                        .map_err(|e2| e2.context((t.failed_with_image_too)(&format!("{e:#}"))))?,
                    Err(e) => return Err(e),
                };
                Ok((model, value))
            }
        }
    }

    /// A single request with the whole folder list returns all fields including the folder.
    fn analyse_at_once(
        &mut self,
        p: &Prompt,
        page: Option<&DynamicImage>,
        progress: &dyn Fn(&str),
        warnings: &mut Vec<String>,
    ) -> Result<AiAnswer> {
        let cfg = p.cfg;
        let t = i18n::analysis::t();
        let (model, answer) = match cfg.llm.backend {
            LlmBackend::Embedded => {
                let llm = self.local(cfg, progress, warnings)?;
                llm.last_stats = None;
                progress(&(t.ai_analyses_on)(&llm.device));
                let system = system_prompt(cfg);
                let schema = llm::response_schema(&schema_spec(cfg, p.folders));
                let page = page.filter(|_| cfg.llm.use_vision);
                let (model, value) =
                    self.ask_document(p, &system, &schema, page, &|chars| user_prompt(p, chars), warnings)?;
                (model, llm::analysis_from_value(value)?)
            }
            LlmBackend::Server => {
                progress(t.ai_analyses);
                ask_llm(p, page)?
            }
        };
        let stats = match cfg.llm.backend {
            LlmBackend::Embedded => self.local.as_ref().and_then(|l| l.last_stats).map(|s| {
                (t.ai_stats)(s.prompt_tokens, s.prompt_time.as_secs_f32(), s.generated, s.generate_time.as_secs_f32())
            }),
            LlmBackend::Server => None,
        };
        Ok(AiAnswer { model, answer, why: Vec::new(), by_words: false, stats })
    }

    /// Run in steps: a short description of the document, then the folder level by level
    /// (by the folder descriptions only), and last the fields for the file name with the
    /// chosen folder as context.
    fn analyse_in_steps(
        &mut self,
        p: &Prompt,
        page: Option<&DynamicImage>,
        progress: &dyn Fn(&str),
        warnings: &mut Vec<String>,
    ) -> Result<AiAnswer> {
        let cfg = p.cfg;
        let t = i18n::analysis::t();
        let page = page.filter(|_| cfg.llm.use_vision);
        match cfg.llm.backend {
            LlmBackend::Embedded => {
                let llm = self.local(cfg, progress, warnings)?;
                progress(&(t.ai_reads_document_on)(&llm.device));
            }
            LlmBackend::Server => progress(t.ai_reads_document),
        }
        let started = Instant::now();
        let (model, value) = self.ask_document(
            p,
            &describe_prompt(cfg),
            &describe_schema(),
            page,
            &|chars| document_text(p, chars),
            warnings,
        )?;
        // Uniform names so that the folder choice recognises personal folders (`Zeugnisse_Max`).
        let summary = naming::canonical_names(value["summary"].as_str().unwrap_or_default().trim(), &cfg.persons);
        let described = Instant::now();

        let about = format!("Worum es geht: {summary}");
        let (folder, why, by_words) = match self.choose_folder(cfg, p.folders, &about, p.text, progress) {
            Ok(choice) => choice,
            Err(e) => {
                warnings.push((t.ai_folder_choice_failed)(&format!("{e:#}")));
                (String::new(), Vec::new(), false)
            }
        };
        let chosen = Instant::now();

        progress(t.ai_determines_fields);
        let place = match folder_note(&cfg.folder_notes, &folder) {
            _ if folder.is_empty() => "noch offen".to_string(),
            Some(note) => format!("{folder} — {note}"),
            None => folder.clone(),
        };
        let context = format!("Worum es geht: {summary}\nAblageort: {place}\n\n");
        let spec = SchemaSpec { without_summary: true, ..schema_spec(cfg, p.folders) };
        // Every request starts without memory: here the AI no longer knows the page image from the description,
        // only its two or three sentences. Sending the image again cost a good 12 s per document in the benchmark
        // of 2026-10-17 (45 documents, Qwen3-VL 8B, AMD RDNA2 card) without making the fields better overall.
        // So it is only sent when there is no text.
        let fields_page = page.filter(|_| p.text.trim().is_empty());
        let (_, value) = self.ask_document(
            p,
            &fields_prompt(cfg, false),
            &llm::response_schema(&spec),
            fields_page,
            &|chars| format!("{context}{}", user_prompt(p, chars)),
            warnings,
        )?;
        let mut answer = llm::analysis_from_value(value)?;
        answer.summary = summary;
        answer.target_folder = folder;
        let stats = (t.step_times)(
            (described - started).as_secs_f32(),
            (chosen - described).as_secs_f32(),
            chosen.elapsed().as_secs_f32(),
        );
        Ok(AiAnswer { model, answer, why, by_words, stats: Some(stats) })
    }

    /// Folder choice level by level: from the document description the AI first picks a top-level
    /// folder, then one of its subfolders and so on, until no subfolder fits better. Folder
    /// keywords (`text` = recognised text) decide beforehand where they are unambiguous. The year
    /// folder logic then picks the year. Returns the folder, the reasons, and whether keywords
    /// decided on every level.
    fn choose_folder(
        &mut self,
        cfg: &Config,
        folders: &[String],
        about: &str,
        text: &str,
        progress: &dyn Fn(&str),
    ) -> Result<(String, Vec<String>, bool)> {
        let t = i18n::analysis::t();
        let hay = archive::normalize(&format!("{text}\n{about}"));
        let mut chosen = String::new();
        let mut why = Vec::new();
        let mut by_words = true;
        for level in 1..=cfg.scan_depth.max(1) {
            let parent = (!chosen.is_empty()).then(|| chosen.clone());
            // Only the subfolders are offered; „keiner“ (none) means the document stays on this level.
            let options = child_options(cfg, folders, parent.as_deref());
            if options.is_empty() {
                break;
            }
            let (pick, reason) = match apply_folder_words(cfg, folders, options, &hay) {
                WordChoice::Decided(path, words) => (path, word_reason(&words)),
                WordChoice::Ask(options) => {
                    by_words = false;
                    progress(&(t.ai_chooses_folder)(level));
                    self.pick(cfg, about, &options, parent.as_deref())?
                }
            };
            if pick.is_empty() {
                if parent.is_some() {
                    why.push((t.no_subfolder_fits)(&reason));
                }
                break;
            }
            let last = archive::split_folder(&pick).last().map(|s| s.to_string()).unwrap_or_default();
            why.push((t.folder_reason)(&last, &reason));
            chosen = pick;
            if archive::year_span(&last).is_some() {
                break;
            }
        }
        let by_words = by_words && !why.is_empty();
        Ok((chosen, why, by_words))
    }

    /// One choice from `options` (path, hint); `parent` is the folder already chosen.
    fn pick(
        &mut self,
        cfg: &Config,
        about: &str,
        options: &[(String, String)],
        parent: Option<&str>,
    ) -> Result<(String, String)> {
        let task = match parent {
            None => format!(
                "Welcher Bereich der Ablage (oberste Ordnerebene) passt am besten? Passt wirklich keiner, \
                 antworte mit „{}“.",
                llm::NO_CHOICE
            ),
            Some(a) => {
                let own = folder_note(&cfg.folder_notes, a).map(|n| format!(" ({n})")).unwrap_or_default();
                format!(
                    "Das Dokument gehört in den Ordner {a}{own}. Welcher seiner Unterordner passt am besten? Passt \
                     keiner, antworte mit „{}“ – dann bleibt das Dokument in {a}.",
                    llm::NO_CHOICE
                )
            }
        };
        let system = format!(
            "Du ordnest Dokumente einer privaten Ablage ein. {task} Entscheide nach dem Thema und den \
             Beschreibungen der Ordner, nicht nach einzelnen Wörtern. Maßgeblich ist das Thema (z. B. Gehalt, \
             Versicherung, Steuer, Gesundheit, Kauf), nicht die Person: Ordner für einzelne Personen gibt es in \
             mehreren Bereichen, sie kommen erst innerhalb des passenden Bereichs. Antworte nur mit JSON: reason \
             (ein kurzer Satz, warum), choice (genau ein Eintrag der Liste oder „{}“).",
            llm::NO_CHOICE
        );
        // Only the folder descriptions count for the folder; the instructions apply to the file name fields.
        let mut user = format!("{about}\n\nZur Auswahl:\n");
        for (path, hint) in options {
            user.push_str("- ");
            user.push_str(path);
            if !hint.is_empty() {
                user.push_str(" — ");
                user.push_str(hint);
            }
            user.push('\n');
        }
        let paths: Vec<String> = options.iter().map(|(p, _)| p.clone()).collect();
        let value = self.ask_json(cfg, &system, &user, &llm::choice_schema(&paths))?;
        let choice = value["choice"].as_str().unwrap_or_default().trim().to_string();
        let reason = value["reason"].as_str().unwrap_or_default().trim().to_string();
        Ok((paths.into_iter().find(|p| *p == choice).unwrap_or_default(), reason))
    }

    /// Short text request with a JSON schema to the already loaded model or the server.
    fn ask_json(
        &mut self,
        cfg: &Config,
        system: &str,
        user: &str,
        schema: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        match cfg.llm.backend {
            LlmBackend::Embedded => {
                let not_loaded = i18n::analysis::t().ai_model_not_loaded;
                let llm = self.local.as_mut().ok_or_else(|| anyhow::anyhow!(not_loaded))?;
                let raw = llm.generate(system, user, None, schema)?;
                llm::extract_json(&raw)
            }
            LlmBackend::Server => {
                let client = LlmClient::new(&cfg.llm);
                let model = client.resolve_model(&cfg.llm.model)?;
                client.complete_json(&model, system, user, None, schema, 300)
            }
        }
    }

    /// Settings from a description of the archive (the *Set up with AI* tab).
    fn setup(&mut self, job: &SetupJob, progress: &dyn Fn(&str)) -> Result<setup::Proposal> {
        let cfg = &job.config;
        let t = i18n::analysis::t();
        if !cfg.llm.enabled {
            anyhow::bail!(t.ai_switched_off);
        }
        let system = setup::system_prompt();
        let user = setup::user_prompt(&job.description, &job.folders.offered);
        let schema = setup::schema();
        let value = match cfg.llm.backend {
            LlmBackend::Embedded => {
                let key = LocalLlm::key_for(&cfg.llm)?;
                let llm = match self.local.take() {
                    Some(llm) if llm.key() == &key => self.local.insert(llm),
                    _ => {
                        progress(t.loading_ai_model);
                        self.local.insert(LocalLlm::load(key, &mut Vec::new())?)
                    }
                };
                progress(&(t.ai_reads_description)(&llm.device));
                // One token is about three characters on average.
                let report = |n: usize| {
                    if n.is_multiple_of(50) {
                        progress(&(t.ai_writes_settings)(n * 3));
                    }
                    !job.cancel.load(Ordering::Relaxed)
                };
                let raw = llm
                    .generate_long(&system, &user, &schema, setup::MAX_ANSWER_TOKENS, setup::MAX_CONTEXT, &report)
                    .map_err(|e| match e.downcast_ref::<PromptTooLong>() {
                        Some(p) => anyhow::anyhow!((t.description_too_long)(
                            p.tokens * 3,
                            p.budget.saturating_sub(setup::MAX_ANSWER_TOKENS) * 3
                        )),
                        None => e,
                    })?;
                llm::extract_json(&raw)?
            }
            LlmBackend::Server => {
                progress(t.ai_server_writes_settings);
                let client = LlmClient::new(&cfg.llm);
                let model = client.resolve_model(&cfg.llm.model)?;
                client.complete_json(&model, &system, &user, None, &schema, setup::MAX_ANSWER_TOKENS)?
            }
        };
        serde_json::from_value(value).map_err(|e| anyhow::anyhow!((t.unexpected_answer)(&e.to_string())))
    }

    fn ocr(&mut self, cfg: &Config, progress: &dyn Fn(&str)) -> Result<&Ocr> {
        match self.ocr.take() {
            Some(ocr) if ocr.config() == &cfg.ocr => Ok(self.ocr.insert(ocr)),
            _ => {
                progress(i18n::analysis::t().starting_ocr);
                Ok(self.ocr.insert(Ocr::new(&cfg.ocr, progress)?))
            }
        }
    }

    fn run(&mut self, job: &Job, progress: &dyn Fn(&str)) -> Result<Analysis> {
        let started = Instant::now();
        let cfg = &job.config;
        let t = i18n::analysis::t();
        let mut warnings = Vec::new();

        progress(t.loading_document);
        let doc = ingest::load_shared(&job.path, cfg.max_pages, progress)?;
        // Per page: if its text layer is usable, it is read; otherwise the page goes to OCR. This way no page is
        // missing from a PDF with both text and image pages, and a broken text layer is not used.
        let has_text: Vec<bool> = (0..doc.pages.len()).map(|i| doc.page_has_text(i)).collect();
        let needs_ocr = has_text.contains(&false);
        // Pages with a text layer can be sideways too (scan apps often put the text over them without rotation,
        // so it does not give them away). OCR aligns such pages like scans – only those, so that PDFs with text
        // stay fast. The line profile used for this needs no OCR.
        let sideways: Vec<bool> = (doc.pages.iter().enumerate())
            .map(|(i, p)| {
                has_text[i] && (doc.tall_text.get(i).copied().unwrap_or(false) || preprocess::looks_sideways(p))
            })
            .collect();

        let ocr = if !needs_ocr && !sideways.contains(&true) {
            None
        } else {
            match self.ocr(cfg, progress) {
                Ok(o) => Some(o),
                // With a text layer on every page, only the alignment is missing; that is not worth a warning.
                Err(_) if !needs_ocr => None,
                Err(e) => {
                    warnings.push((t.ocr_unavailable)(&format!("{e:#}")));
                    None
                }
            }
        };

        let total = doc.pages.len();
        let mut pages = Vec::with_capacity(total);
        let mut texts = Vec::new();
        let (mut from_layer, mut from_ocr) = (false, false);
        let mut corrections = Vec::new();
        let mut rotations = vec![0; total];
        for (i, page) in doc.pages.iter().enumerate() {
            let label = (t.page)(i + 1, total);
            let layer = doc.page_texts.get(i).map(String::as_str).unwrap_or_default();
            let image = if has_text[i] && !sideways[i] {
                page.clone()
            } else {
                progress(&(t.detecting_orientation)(&label));
                let probe = |img: &image::GrayImage| ocr.and_then(|o| o.recognize_confident(img).ok());
                let fixed = preprocess::correct_page(page, sideways[i], probe);
                if fixed.rotation != 0 || fixed.skew != 0.0 {
                    corrections.push(describe_correction(i + 1, fixed.rotation, fixed.skew));
                }
                rotations[i] = fixed.rotation;
                fixed.image
            };
            let (text, by_ocr) = if has_text[i] {
                (layer.trim().to_string(), false)
            } else {
                if ingest::garbled_text(layer) {
                    warnings.push((t.garbled_text_layer)(&label));
                }
                let found = ocr.and_then(|o| {
                    progress(&(t.recognising_text)(&label));
                    let gray = preprocess::enhance_for_ocr(&limit_size(&image, OCR_MAX_SIDE));
                    o.recognize(&gray).map_err(|e| warnings.push((t.ocr_failed)(&label, &format!("{e:#}")))).ok()
                });
                page_text(layer, found.as_deref())
            };
            if !text.is_empty() {
                from_ocr |= by_ocr;
                from_layer |= !by_ocr;
                texts.push(text);
            }
            pages.push(image);
        }
        // Where the text comes from; without any text found, the OCR engine that tried.
        let ocr_engine = match doc.kind {
            DocKind::Pdf | DocKind::Image => {
                let engine = ocr.filter(|_| from_ocr || (needs_ocr && !from_layer)).map(Ocr::name);
                match (from_layer, engine) {
                    (true, Some(engine)) => Some((t.text_layer_and_ocr)(&engine)),
                    (true, None) => Some(t.pdf_text_layer.to_string()),
                    (false, engine) => engine,
                }
            }
            other => Some(other.label().to_string()),
        };
        let text = if doc.kind.has_pages() { texts.join("\n\n") } else { doc.text_layer.clone() };
        let text = heuristics::collapse_letter_spacing(&text);
        if text.trim().is_empty() {
            warnings.push(match doc.kind {
                DocKind::Other => t.no_readable_content.into(),
                _ => t.no_text.into(),
            });
        }
        if doc.kind == DocKind::LegacyOffice {
            warnings.push(t.legacy_office.into());
        }

        let filename = job.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let stem = original_stem(&job.path);
        let today = Local::now().date_naive();
        let mut suggestion = heuristic_suggestion(cfg, &text, &stem, &job.folders.offered, today);
        let mut ai_stats = None;

        if cfg.llm.enabled {
            let prompt =
                Prompt { cfg, folders: &job.folders.offered, text: &text, filename: &filename, kind: doc.kind };
            let answer = if cfg.llm.folder_steps {
                self.analyse_in_steps(&prompt, pages.first(), progress, &mut warnings)
            } else {
                self.analyse_at_once(&prompt, pages.first(), progress, &mut warnings)
            };
            match answer {
                Ok(ai) => {
                    // If keywords decided on every level, the folder comes from them.
                    let by_words = ai.by_words;
                    suggestion.folder_why = ai.why;
                    ai_stats = ai.stats;
                    let ctx = MergeContext { cfg, folders: &job.folders, text: &text, today };
                    merge_llm(&mut suggestion, &ai.answer, &ai.model, &ctx, &mut warnings);
                    if by_words && suggestion.origin.folder == Origin::Ai {
                        suggestion.origin.folder = Origin::Words;
                    }
                }
                Err(e) => warnings.push((t.ai_not_used)(&format!("{e:#}"))),
            }
        }

        finalize(&mut suggestion, cfg, &job.folders, &text, today);
        let ext = ingest::extension(&job.path).unwrap_or_default();
        suggestion.keep_name = naming::keeps_name(&ext, &cfg.keep_name_extensions);

        let previews = pages.iter().map(to_preview).collect();
        Ok(Analysis {
            suggestion,
            kind: doc.kind,
            text,
            previews,
            page_count: doc.page_count,
            ocr_engine,
            corrections,
            rotations,
            warnings,
            duration: started.elapsed(),
            ai_stats,
        })
    }
}

fn describe_correction(page: usize, rotation: u16, skew: f32) -> String {
    let t = i18n::analysis::t();
    let mut parts = Vec::new();
    if rotation != 0 {
        parts.push((t.rotated)(rotation));
    }
    if skew != 0.0 {
        parts.push((t.straightened)(skew.abs()));
    }
    (t.correction)(page, &parts.join(", "))
}

/// Text of a page without a usable text layer: what OCR found. If OCR finds no more than the short text layer
/// (say, only a closing phrase), the layer's text stays; a broken text layer never counts. Returns the text and
/// whether it comes from OCR.
fn page_text(layer: &str, ocr: Option<&str>) -> (String, bool) {
    let layer = if ingest::garbled_text(layer) { "" } else { layer.trim() };
    match ocr.map(str::trim) {
        Some(found) if ingest::count_readable(found).1 > ingest::count_readable(layer).1 => (found.to_string(), true),
        _ => (layer.to_string(), false),
    }
}

fn limit_size(img: &DynamicImage, max_side: u32) -> DynamicImage {
    if img.width().max(img.height()) > max_side {
        img.resize(max_side, max_side, image::imageops::FilterType::Triangle)
    } else {
        img.clone()
    }
}

fn to_preview(img: &DynamicImage) -> PreviewPage {
    let rgba = limit_size(img, PREVIEW_MAX_SIDE).to_rgba8();
    PreviewPage { size: [rgba.width() as usize, rgba.height() as usize], rgba: rgba.into_raw() }
}

/// Suggestion without AI: date (also from the file name), type, sender and subject from the text; the folder
/// by the folder keywords, otherwise by similarity of the folder names. Also used when the AI fails.
pub fn heuristic_suggestion(cfg: &Config, text: &str, stem: &str, folders: &[String], today: NaiveDate) -> Suggestion {
    let doctype = heuristics::doctype(text).map(|d| naming::canonical_doctype(d, &cfg.doctypes)).unwrap_or_default();
    let sender = heuristics::sender(text).map(|s| naming::normalize_sender(&s, &cfg.senders)).unwrap_or_default();
    let subject = heuristics::subject(text).unwrap_or_default();
    let (mut folder, folder_why) = folder_by_words(cfg, folders, &archive::normalize(text));
    let mut folder_origin = if folder.is_empty() { Origin::None } else { Origin::Words };
    if folder.is_empty() {
        folder = archive::best_folder_match(folders, &[&sender, &doctype, &subject]).unwrap_or_default();
        if !folder.is_empty() {
            folder_origin = Origin::Text;
        }
    }
    let from_text =
        heuristics::document_date(text, today).map(DocDate::full).or_else(|| heuristics::month_year(text, today));
    let date = from_text.or_else(|| heuristics::filename_date(stem));
    let filled = |v: &str| if v.is_empty() { Origin::None } else { Origin::Text };
    let origin = Origins {
        date: match (from_text, date) {
            (Some(_), _) => Origin::Text,
            (None, Some(_)) => Origin::Filename,
            _ => Origin::None,
        },
        doctype: filled(&doctype),
        sender: filled(&sender),
        subject: filled(&subject),
        person: Origin::None,
        extra: Origin::None,
        folder: folder_origin,
    };
    Suggestion {
        date,
        doctype,
        sender,
        subject,
        summary: heuristics::summary(text),
        folder,
        folder_why,
        confidence: 0.3,
        source: "heuristics".into(),
        origin,
        ..Default::default()
    }
}

/// Folder by keywords alone: level by level, as long as exactly one folder matches on the level.
fn folder_by_words(cfg: &Config, folders: &[String], hay: &str) -> (String, Vec<String>) {
    let mut chosen = String::new();
    let mut why = Vec::new();
    for _ in 0..cfg.scan_depth.max(1) {
        let parent = (!chosen.is_empty()).then(|| chosen.clone());
        let options = child_options(cfg, folders, parent.as_deref());
        if options.is_empty() {
            break;
        }
        let WordChoice::Decided(pick, words) = apply_folder_words(cfg, folders, options, hay) else { break };
        let last = archive::split_folder(&pick).last().map(|s| s.to_string()).unwrap_or_default();
        why.push((i18n::analysis::t().folder_reason)(&last, &word_reason(&words)));
        chosen = pick;
        if archive::year_span(&last).is_some() {
            break;
        }
    }
    (chosen, why)
}

fn word_reason(words: &[String]) -> String {
    (i18n::analysis::t().keywords_found)(words)
}

fn schema_spec<'a>(cfg: &'a Config, folders: &'a [String]) -> SchemaSpec<'a> {
    SchemaSpec {
        doctypes: &cfg.doctypes,
        extras: &cfg.extras,
        persons: cfg.persons.iter().map(|p| p.name.clone()).collect(),
        folders: (!cfg.allow_new_folders).then_some(folders),
        without_folder: cfg.llm.folder_steps,
        without_summary: false,
    }
}

fn system_prompt(cfg: &Config) -> String {
    fields_prompt(cfg, true)
}

/// Instructions for the file name fields; `with_summary` asks for a summary first.
fn fields_prompt(cfg: &Config, with_summary: bool) -> String {
    let folder_rule = if cfg.allow_new_folders {
        "Wähle bevorzugt exakt einen Pfad aus der Liste. Nur wenn wirklich keiner passt, schlage einen neuen, \
         kurzen Unterordner im gleichen Stil vor (mit / getrennt)."
    } else {
        "Wähle exakt einen Pfad aus der Liste (Schreibweise übernehmen). Passt keiner, gib einen leeren String zurück."
    };
    let doctype_rule = if cfg.doctypes.is_empty() {
        "Dokumentart als ein deutsches Substantiv in der Einzahl, z. B. Rechnung, Bescheid, Kontoauszug.".to_string()
    } else {
        let families: Vec<String> = naming::doctype_families(&cfg.doctypes)
            .into_iter()
            .map(|(general, specific)| format!("{general} → {}", specific.join(", ")))
            .collect();
        let families = if families.is_empty() {
            String::new()
        } else {
            format!(" Genauere Begriffe der Liste: {}.", families.join("; "))
        };
        format!(
            "Ein Begriff aus der vorgegebenen Liste, in der Einzahl: der genaueste, der die Art dieses Dokuments \
             wirklich bezeichnet.{families} Bezeichnet kein Begriff der Liste die Art genau – nur ein allgemeiner \
             wie Schreiben, Mitteilung oder Bescheinigung würde irgendwie passen –, dann „{other}“ und in \
             doctype_new die genaue Art als ein deutsches Substantiv in der Einzahl (z. B. Meldebescheinigung). \
             Sonst ist doctype_new leer.",
            other = llm::DOCTYPE_OTHER
        )
    };
    let persons = if cfg.persons.is_empty() {
        "Name der Person, die das Dokument betrifft, sonst leer.".to_string()
    } else {
        let names: Vec<&str> = cfg.persons.iter().map(|p| p.name.as_str()).collect();
        format!(
            "Wen das Dokument persönlich betrifft: {}. Nur den Vornamen in genau dieser Schreibweise, ohne \
             Nachnamen. Mehrere mit - verbunden (z. B. {}), alle zusammen: Familie. Andere Personen als \
             Vorname-Nachname. Nur angeben, wenn es um diese Person selbst geht (Gehalt, Gesundheit, Zeugnis, \
             Urkunde, Schule, ihr eigenes Konto oder ihre Mitgliedschaft). Die Anschrift allein genügt nicht: \
             Rechnungen, Versicherungen, Konten, Steuern, Haus und Auto des Haushalts bleiben ohne Person (leer).",
            names.join(", "),
            names.iter().take(2).copied().collect::<Vec<_>>().join("-")
        )
    };
    let extras = if cfg.extras.is_empty() { "leer".to_string() } else { cfg.extras.join(", ") };
    let folder_line = if cfg.llm.folder_steps {
        String::new()
    } else {
        format!("- target_folder: {folder_rule} Entscheide nach dem Thema und den Ordnerbeschreibungen.\n")
    };
    let summary_line = if with_summary {
        "- summary: ein bis zwei Sätze auf Deutsch: was ist es, von wem, wichtige Beträge oder Fristen.\n"
    } else {
        ""
    };
    let mut p = format!(
        "Du ordnest Dokumente einer privaten Ablage ein. Du bekommst den erkannten Text (aus OCR, kann Fehler \
         und fehlende Umlaute enthalten), oft das Seitenbild und den ursprünglichen Dateinamen. \
         Antworte ausschließlich mit einem JSON-Objekt:\n\
         {summary_line}\
         - doctype: {doctype_rule}\n\
         - sender: Wer das Dokument ausgestellt hat, als kurzer Alltagsname ohne Rechtsform (GmbH, AG, e.V.), \
           z. B. Finanzamt, HUK, Zahnarzt Weber. Bei eigenen Schreiben der Empfänger. Ist der Name unbekannt, \
           die Rolle (Arzt, Labor, Schule, Standesamt, Verein).\n\
         - date: Das Datum, das auf dem Dokument steht (Brief-, Rechnungs- oder Ausstellungsdatum) als YYYY-MM-DD. \
           NICHT Fälligkeit, Leistungszeitraum, Druckdatum einer Vorlage oder Geburtsdatum. Fehlt der Tag: YYYY-MM, \
           fehlt auch der Monat: YYYY. Unbekannt: leer. Nie ein Datum erfinden.\n\
         - subject: Betreff, ein bis vier Wörter, nur wenn er etwas unterscheidet. Zeiträume stehen vorn: Jahr YYYY, \
           Monat YYYY-MM, Quartal YYYY-Q1; bei Steuern das Steuerjahr. Fahrzeuge vorn (z. B. 320d-Bremsen).\n\
         - person: {persons}\n\
         - extra: einer von: {extras} – nur wenn es eindeutig zutrifft (z. B. von Hand unterschrieben, als \
           Entwurf oder Kopie gekennzeichnet), nicht bei maschinell erstellten Schreiben; im Zweifel leer.\n\
         {folder_line}\
         - confidence: Zahl zwischen 0 und 1, wie sicher du dir insgesamt bist.\n\
         Korrigiere offensichtliche OCR-Fehler (z. B. fehlende Umlaute) in deinen Antworten."
    );
    if !cfg.llm.instructions.trim().is_empty() {
        p.push_str("\n\nRegeln dieser Ablage:\n");
        p.push_str(cfg.llm.instructions.trim());
    }
    p
}

fn user_prompt(p: &Prompt, max_chars: usize) -> String {
    let mut out = String::new();
    if !p.cfg.llm.folder_steps {
        out.push_str("Vorhandene Ordner der Ablage:\n");
    }
    if p.folders.is_empty() && !p.cfg.llm.folder_steps {
        out.push_str("(noch keine Ordner vorhanden)\n");
    }
    for f in p.folders.iter().take(if p.cfg.llm.folder_steps { 0 } else { MAX_FOLDERS_IN_PROMPT }) {
        out.push_str("- ");
        out.push_str(f);
        if let Some(note) = folder_note(&p.cfg.folder_notes, f) {
            out.push_str(" — ");
            out.push_str(note);
        }
        out.push('\n');
    }
    if !p.cfg.doctypes.is_empty() {
        out.push_str("\nDokumentarten: ");
        out.push_str(&p.cfg.doctypes.join(", "));
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&document_text(p, max_chars));
    out
}

/// Original file name and recognised text (at most `max_chars` characters).
fn document_text(p: &Prompt, max_chars: usize) -> String {
    // The instructions to the AI stay German, including the file type.
    let kind = p.kind.label_in(&i18n::files::DE);
    let mut out = format!("Ursprünglicher Dateiname: {} ({kind})\n", p.filename);
    let text: String = p.text.chars().take(max_chars).collect();
    if text.trim().is_empty() {
        out.push_str(if p.kind.has_pages() {
            "\nEs wurde kein Text erkannt. Werte bitte das Seitenbild aus.\n"
        } else {
            "\nDer Inhalt ist nicht lesbar. Schließe aus dem Dateinamen auf Datum, Art und Ordner.\n"
        });
    } else {
        out.push_str("\nErkannter Text:\n<<<\n");
        out.push_str(&text);
        out.push_str("\n>>>\n");
    }
    out
}

/// Instructions for the short description that the AI then uses to choose the folder.
fn describe_prompt(cfg: &Config) -> String {
    let mut p = "Du liest ein Dokument aus einer privaten Ablage. Du bekommst den erkannten Text (aus OCR, kann \
                 Fehler enthalten), oft das Seitenbild und den ursprünglichen Dateinamen. Beschreibe in zwei bis drei \
                 Sätzen auf Deutsch: was für ein Dokument es ist, wer es ausgestellt hat, an wen es gerichtet ist und \
                 worum es geht (Gegenstand, Vertrag, Konto, Fahrzeug, Zeitraum, Beträge). Antworte nur mit JSON: \
                 summary."
        .to_string();
    if !cfg.persons.is_empty() {
        let names: Vec<String> = cfg
            .persons
            .iter()
            .map(|p| match p.aliases.is_empty() {
                true => p.name.clone(),
                false => format!("{} (auch {})", p.name, p.aliases.join(", ")),
            })
            .collect();
        p.push_str(&format!(
            " Personen des Haushalts: {}. Nenne sie nur mit dem ersten Namen, ohne Nachnamen.",
            names.join("; ")
        ));
    }
    p
}

fn describe_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": { "summary": { "type": "string", "description": "Zwei bis drei Sätze: was, von wem, worum, für wen" } },
        "required": ["summary"],
        "additionalProperties": false
    })
}

/// Options for one level of the folder choice: the direct subfolders of `parent` (without `parent`
/// the top level), each with its description and the names of its subfolders. Year folders are
/// offered only when there are several series (`2026_Max`, `2026_Erika`): the newest year of each
/// series; the year folder logic then picks the right one. Decade bundles are left out.
fn child_options(cfg: &Config, folders: &[String], parent: Option<&str>) -> Vec<(String, String)> {
    let children = direct_children(folders, parent);
    let join = |seg: &str| match parent {
        Some(p) => format!("{p}/{seg}"),
        None => seg.to_string(),
    };
    let mut options = Vec::new();
    // The newest year per series (text before and after the year).
    let mut series: Vec<(String, String, i32, String)> = Vec::new();
    for seg in &children {
        if archive::is_decade_bundle(seg) {
            continue;
        }
        match archive::year_span(seg) {
            Some((a, b, year)) => {
                let key = (archive::normalize(&seg[..a]), archive::normalize(&seg[b..]));
                match series.iter_mut().find(|s| (s.0.as_str(), s.1.as_str()) == (key.0.as_str(), key.1.as_str())) {
                    Some(s) if s.2 < year => (s.2, s.3) = (year, seg.clone()),
                    Some(_) => {}
                    None => series.push((key.0, key.1, year, seg.clone())),
                }
            }
            None => {
                let path = join(seg);
                let hint = folder_hint(cfg, folders, &path);
                options.push((path, hint));
            }
        }
    }
    if series.len() > 1 {
        for (_, _, _, seg) in series {
            let path = join(&seg);
            let note = folder_note(&cfg.folder_notes, &path).unwrap_or_default();
            options.push((path, format!("Jahresordner {note}").trim().to_string()));
        }
    }
    options
}

/// Result of the keywords on one level of the folder choice.
#[derive(Debug, PartialEq)]
enum WordChoice {
    /// Exactly one folder has a match: it applies without AI (folder, keywords found).
    Decided(String, Vec<String>),
    /// The AI chooses from these folders; matches are listed in their hint.
    Ask(Vec<(String, String)>),
}

/// Folder keywords on one level: if exactly one folder matches, it decides; if several match, the AI
/// chooses and sees the matches. Matches of a subfolder also count for its parent folders:
/// „Haftpflicht“ in `Versicherung/Haftpflicht` already leads to `Versicherung` on the top level.
/// A negative word cancels the keywords of its folder (including subfolders); the folder is still
/// offered, and the AI then decides as without keywords.
fn apply_folder_words(cfg: &Config, folders: &[String], options: Vec<(String, String)>, hay: &str) -> WordChoice {
    if cfg.folder_words.is_empty() {
        return WordChoice::Ask(options);
    }
    let key = |p: &str| archive::normalize(&archive::split_folder(p).join("/"));
    let below: Vec<(String, Vec<String>)> = cfg
        .folder_words
        .iter()
        .filter(|(k, _)| words_folder_exists(folders, k))
        .filter_map(|(k, v)| {
            let (plus, minus) = archive::FolderWords::parse(v).hits(hay);
            (!minus && !plus.is_empty()).then(|| (key(k), plus))
        })
        .collect();
    let hits: Vec<Vec<String>> = options
        .iter()
        .map(|(path, _)| {
            let (mut plus, minus) = folder_words(cfg, path).map(|w| w.hits(hay)).unwrap_or_default();
            if minus {
                return Vec::new();
            }
            let prefix = format!("{}/", key(path));
            for word in below.iter().filter(|(k, _)| k.starts_with(&prefix)).flat_map(|(_, p)| p) {
                if !plus.contains(word) {
                    plus.push(word.clone());
                }
            }
            plus
        })
        .collect();
    let mut found = options.iter().zip(&hits).filter(|(_, plus)| !plus.is_empty());
    if let (Some(((path, _), plus)), None) = (found.next(), found.next()) {
        return WordChoice::Decided(path.clone(), plus.clone());
    }
    WordChoice::Ask(
        options
            .into_iter()
            .zip(hits)
            .map(|((path, hint), plus)| match plus.is_empty() {
                true => (path, hint),
                false => {
                    let found = format!("im Dokument: {}", plus.join(", "));
                    let hint = if hint.is_empty() { found } else { format!("{hint} [{found}]") };
                    (path, hint)
                }
            })
            .collect(),
    )
}

/// Keywords of a folder; for year folders the key with `{year}` also applies (`Gehalt/{year}_Max`).
fn folder_words(cfg: &Config, folder: &str) -> Option<archive::FolderWords> {
    let mut keys = vec![folder.to_string()];
    let parts = archive::split_folder(folder);
    if let Some((last, parents)) = parts.split_last()
        && let Some((a, b, _)) = archive::year_span(last)
    {
        let pattern = format!("{}{{year}}{}", &last[..a], &last[b..]);
        keys.push(parents.iter().copied().chain([pattern.as_str()]).collect::<Vec<_>>().join("/"));
    }
    keys.iter()
        .find_map(|k| folder_note(&cfg.folder_words, k))
        .map(archive::FolderWords::parse)
        .filter(|w| !w.is_empty())
}

/// Does the folder of a keyword entry exist (for `{year}`, the parent folder)? Keywords of unused
/// folders do not count.
fn words_folder_exists(folders: &[String], entry: &str) -> bool {
    let parts = archive::split_folder(entry);
    let upto = parts.iter().position(|p| p.contains("{year}")).unwrap_or(parts.len());
    upto == 0 || archive::find_existing_folder(folders, &parts[..upto].join("/")).is_some()
}

/// Names of the direct subfolders (without `parent` the top level), in list order.
fn direct_children(folders: &[String], parent: Option<&str>) -> Vec<String> {
    let depth = parent.map_or(0, |p| archive::split_folder(p).len());
    let parent_n = parent.map(|p| archive::normalize(&archive::split_folder(p).join("/")));
    let mut out: Vec<String> = Vec::new();
    for f in folders {
        let parts = archive::split_folder(f);
        if parts.len() <= depth {
            continue;
        }
        if let Some(pn) = &parent_n
            && archive::normalize(&parts[..depth].join("/")) != *pn
        {
            continue;
        }
        let seg = parts[depth];
        if !out.iter().any(|o| o == seg) {
            out.push(seg.to_string());
        }
    }
    out
}

/// Hint for a folder: its own description and its subfolders (without years) with their descriptions –
/// this way the AI already sees on the upper level where e.g. „Meldebescheinigung“ or „Telekom“ belong.
/// Folders of single persons (`Max`, `Zeugnisse_Max`) appear only as „je Person einer“ (one per person), so that
/// a name in the document does not tip the balance on this level already.
fn folder_hint(cfg: &Config, folders: &[String], folder: &str) -> String {
    let (personal, children): (Vec<String>, Vec<String>) = direct_children(folders, Some(folder))
        .into_iter()
        .filter(|c| archive::year_span(c).is_none() && !archive::is_decade_bundle(c))
        .partition(|c| names_person(c, &cfg.persons));
    let mut parts: Vec<String> = children
        .iter()
        .take(10)
        .map(|c| match folder_note(&cfg.folder_notes, &format!("{folder}/{c}")) {
            Some(note) => format!("{c}: {note}"),
            None => c.clone(),
        })
        .collect();
    if !personal.is_empty() {
        parts.push("je Person einer".into());
    }
    let mut hint = folder_note(&cfg.folder_notes, folder).unwrap_or_default().to_string();
    if !parts.is_empty() {
        if !hint.is_empty() {
            hint.push(' ');
        }
        hint.push_str(&format!("(Unterordner: {})", parts.join("; ")));
    }
    hint
}

/// Does the folder name contain a person's name (`Max`, `Zeugnisse_Max`)?
fn names_person(folder: &str, persons: &[crate::config::Alias]) -> bool {
    folder.split(['_', '-', ' ']).any(|w| {
        let w = archive::normalize(w);
        persons.iter().any(|p| p.matches_word(&w))
    })
}

/// Description of a folder (key compared regardless of case and separators).
pub fn folder_note<'a>(notes: &'a std::collections::BTreeMap<String, String>, folder: &str) -> Option<&'a str> {
    let wanted = archive::normalize(&archive::split_folder(folder).join("/"));
    notes
        .iter()
        .find(|(k, _)| archive::normalize(&archive::split_folder(k).join("/")) == wanted)
        .map(|(_, v)| v.as_str())
        .filter(|v| !v.trim().is_empty())
}

fn encode_jpeg_b64(img: &DynamicImage) -> Result<String> {
    let rgb = limit_size(img, VISION_MAX_SIDE).to_rgb8();
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(Cursor::new(&mut buf), 85).encode_image(&rgb)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(buf))
}

fn ask_llm(p: &Prompt, first_page: Option<&DynamicImage>) -> Result<(String, LlmAnalysis)> {
    let cfg = p.cfg;
    let client = LlmClient::new(&cfg.llm);
    let model = client.resolve_model(&cfg.llm.model)?;
    let image = match (cfg.llm.use_vision, first_page) {
        (true, Some(img)) => Some(encode_jpeg_b64(img)?),
        _ => None,
    };
    let system = system_prompt(cfg);
    let user = user_prompt(p, cfg.llm.max_chars);
    let schema = llm::response_schema(&schema_spec(cfg, p.folders));
    let mut req =
        LlmRequest { model: &model, system: &system, user: &user, image_jpeg_b64: image.as_deref(), schema: &schema };
    let answer = match client.analyze(&req) {
        Ok(a) => a,
        // Text models without image support reject image requests; then retry with text only.
        Err(e) if req.image_jpeg_b64.is_some() && !p.text.trim().is_empty() => {
            req.image_jpeg_b64 = None;
            let also_failed = i18n::analysis::t().failed_with_image_too;
            client.analyze(&req).map_err(|e2| e2.context(also_failed(&format!("{e:#}"))))?
        }
        Err(e) => return Err(e),
    };
    Ok((model, answer))
}

/// Date from a form field or an AI answer (also partial).
pub fn parse_date(s: &str) -> Option<DocDate> {
    DocDate::parse(s)
}

/// Takes the AI's date only if it matches the text. Small models like to mix up years or pick deadlines
/// and periods. The same rule applies as without AI (`heuristics::document_date`): a letter date
/// („Ort, den …“, „Datum: …“) takes precedence, among several the one closest to today. On the other
/// ranks the AI decides if its date is in the text on the clearest rank.
fn reconcile_date(heuristic: Option<DocDate>, llm: Option<DocDate>, text: &str, today: NaiveDate) -> Option<DocDate> {
    let Some(llm) = llm else { return heuristic };
    let Some(heur) = heuristic else { return Some(llm) };
    if text.trim().is_empty() {
        return Some(llm);
    }
    let Some(llm_full) = llm.to_naive() else {
        // Partial date from the AI: a date from the text that matches it (year and, if given, month) is more precise.
        // A different one often comes from the fine print, such as a court ruling on the back of a payslip.
        let fits = heur.year == llm.year && (llm.month.is_none() || heur.month == llm.month);
        return if fits && (heur.is_full() || llm.month.is_none()) { Some(heur) } else { Some(llm) };
    };
    let found = heuristics::find_dates(text, today);
    let Some((rank, best)) = heuristics::best_date(&found, today) else { return Some(heur) };
    let llm_rank = found.iter().filter(|f| f.date == llm_full).map(heuristics::FoundDate::rank).min();
    if rank != DateRank::Letter && llm_rank == Some(rank) { Some(llm) } else { Some(DocDate::full(best)) }
}

struct MergeContext<'a> {
    cfg: &'a Config,
    folders: &'a FolderIndex,
    text: &'a str,
    today: NaiveDate,
}

fn merge_llm(s: &mut Suggestion, a: &LlmAnalysis, model: &str, ctx: &MergeContext, warnings: &mut Vec<String>) {
    let cfg = ctx.cfg;
    let llm_date = parse_date(&a.date);
    let date = reconcile_date(s.date, llm_date, ctx.text, ctx.today);
    if date != s.date || (llm_date.is_some() && date == llm_date) {
        s.origin.date = if date.is_some() { Origin::Ai } else { Origin::None };
    }
    s.date = date;
    // „andere“ (other): no term in the list fits and the AI gives its own; it stays instead of falling back
    // to the closest list term.
    let own = a.doctype == llm::DOCTYPE_OTHER;
    let raw_doctype = if own { a.doctype_new.trim() } else { a.doctype.trim() };
    if !raw_doctype.is_empty() {
        let doctype = if own {
            naming::own_doctype(raw_doctype, &cfg.doctypes)
        } else {
            naming::canonical_doctype(raw_doctype, &cfg.doctypes)
        };
        // The heuristics sometimes know the more precise term of the same type (Bescheid → Gebührenbescheid).
        if !naming::refines(&s.doctype, &doctype, &cfg.doctypes) {
            s.doctype = doctype;
            s.origin.doctype = Origin::Ai;
        }
    }
    if !a.sender.is_empty() {
        s.sender = naming::normalize_sender(&a.sender, &cfg.senders);
        s.origin.sender = Origin::Ai;
    }
    s.subject = naming::normalize_period(&a.subject);
    s.origin.subject = if s.subject.is_empty() { Origin::None } else { Origin::Ai };
    s.person = naming::normalize_person(&a.person, &cfg.persons);
    s.origin.person = if s.person.is_empty() { Origin::None } else { Origin::Ai };
    s.extra = cfg
        .extras
        .iter()
        .find(|e| archive::normalize(e) == archive::normalize(&a.extra))
        .filter(|e| extra_in_text(e, ctx.text))
        .cloned()
        .unwrap_or_default();
    s.origin.extra = if s.extra.is_empty() { Origin::None } else { Origin::Ai };
    if !a.summary.is_empty() {
        s.summary = a.summary.clone();
    }
    let wanted = a.target_folder.trim_matches(['/', '\\', ' ']);
    if let Some(existing) = archive::find_existing_folder(&ctx.folders.all, wanted) {
        s.folder = existing.clone();
        s.origin.folder = Origin::Ai;
    } else if !wanted.is_empty() {
        if cfg.allow_new_folders && archive::safe_relative_folder(wanted).is_ok() {
            s.folder = wanted.replace('\\', "/");
            s.origin.folder = Origin::Ai;
        } else {
            warnings.push((i18n::analysis::t().unknown_folder_replaced)(wanted));
        }
    }
    s.confidence_from_ai = a.confidence > 0.0;
    s.confidence = if a.confidence > 0.0 { a.confidence } else { 0.6 };
    s.source = format!("AI ({model})");
}

/// Is the extra in the document („Kopie“, „Entwurf“, „Anlage 1“)? Only then does the AI's suggestion apply –
/// otherwise it sets it far too often. Negated, it does not count: machine-made letters often say „wird nicht
/// unterschrieben“ (will not be signed) in the fine print. What the text does not show (signed by hand, scanned in
/// black and white) the user picks themselves.
fn extra_in_text(extra: &str, text: &str) -> bool {
    let negations = crate::doclang::current().negations;
    let words = |s: &str| -> Vec<String> {
        archive::normalize(s)
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(String::from)
            .collect()
    };
    let (hay, wanted) = (words(text), words(extra));
    !wanted.is_empty()
        && hay.windows(wanted.len()).enumerate().any(|(i, found)| {
            found == wanted.as_slice() && !hay[i.saturating_sub(2)..i].iter().any(|w| negations.contains(&w.as_str()))
        })
}

/// Applies rules, adjusts year folders, finds decade bundles, and falls back to the fallback folder if needed.
fn finalize(s: &mut Suggestion, cfg: &Config, folders: &FolderIndex, text: &str, today: NaiveDate) {
    let t = i18n::analysis::t();
    let root = cfg.archive_root();
    let (ruled, rule_person) = apply_rules(s, cfg, text);
    let year = naming::reference_year(&s.subject, s.date);
    if !ruled
        && let Some(y) = year
        && let Some(adjusted) = archive::adjust_year_folder(&root, &s.folder, y, &folders.all, today.year())
    {
        s.folder_notes.push((t.year_folder)(y));
        s.folder = adjusted;
    }
    if !s.folder.trim().is_empty() {
        let located = archive::locate_folder(&root, &s.folder, &folders.all);
        if located != s.folder {
            s.folder_notes.push(t.in_decade_folder.into());
            s.folder = located;
        }
    }
    if s.folder.trim().is_empty() {
        s.folder = cfg.fallback_folder.clone();
        s.folder_notes.push(t.nothing_found.into());
        s.origin.folder = Origin::Fallback;
    }
    if cfg.person_from_folder && !rule_person {
        let person = naming::person_in_folder(&s.person, &s.folder, &cfg.persons);
        if person != s.person {
            s.person = person;
            s.origin.person = Origin::Folder;
        }
    }
}

/// Applies the first matching rule. Returns whether the rule set the folder and whether it set the person.
fn apply_rules(s: &mut Suggestion, cfg: &Config, text: &str) -> (bool, bool) {
    let hay = format!("{text}\n{}\n{}", s.sender, s.doctype);
    let Some(rule) = archive::match_rule(&cfg.rules, &hay) else { return (false, false) };
    if let Some(d) = rule.doctype.as_ref().filter(|d| !d.is_empty()) {
        s.doctype = naming::canonical_doctype(d, &cfg.doctypes);
        s.origin.doctype = Origin::Rule;
    }
    if let Some(snd) = rule.sender.as_ref().filter(|d| !d.is_empty()) {
        s.sender = snd.clone();
        s.origin.sender = Origin::Rule;
    }
    let rule_person = rule.person.as_ref().is_some_and(|p| !p.is_empty());
    if let Some(p) = rule.person.as_ref().filter(|d| !d.is_empty()) {
        s.person = naming::normalize_person(p, &cfg.persons);
        s.origin.person = Origin::Rule;
    }
    let mut year = naming::reference_year(&s.subject, s.date);
    let mut folder = archive::render_folder(&rule.folder, s.date, year);
    // New year folders only for the current and the previous year; older ones only if they exist.
    if let Some(y) = year
        && y < Local::now().year() - 1
        && rule.folder.contains("{year}")
    {
        let located = archive::locate_folder(&cfg.archive_root(), &folder, &[]);
        let exists = cfg.archive_root().join(archive::safe_relative_folder(&located).unwrap_or_default()).is_dir();
        if !exists {
            s.folder_notes.push((i18n::analysis::t().no_year_folder)(y));
            year = None;
            folder = archive::render_folder(&rule.folder, s.date, year);
        }
    }
    s.rule = Some((i18n::analysis::t().rule_applied)(&rule.display_name(), &rule.folder));
    if folder.is_empty() {
        return (false, rule_person);
    }
    s.folder = folder;
    s.origin.folder = Origin::Rule;
    (true, rule_person)
}

/// File name for a suggestion: the target folder's template (if set), otherwise the general one.
pub fn suggested_filename(cfg: &Config, f: &naming::TemplateFields, folder: &str) -> String {
    let template = folder_template(cfg, folder).unwrap_or(&cfg.filename_template);
    naming::render_filename(
        template,
        &cfg.date_format,
        f,
        naming::NameRules { compact: cfg.compact_filenames, max_len: cfg.max_filename_length },
    )
}

/// Separate template for a folder or one of its parent folders (the most specific one wins).
pub fn folder_template<'a>(cfg: &'a Config, folder: &str) -> Option<&'a String> {
    let f = archive::normalize(&archive::split_folder(folder).join("/"));
    cfg.folder_templates
        .iter()
        .filter(|t| {
            let t = archive::normalize(&archive::split_folder(&t.folder).join("/"));
            !t.is_empty() && (f == t || f.strip_prefix(t.as_str()).is_some_and(|r| r.starts_with('/')))
        })
        .max_by_key(|t| t.folder.len())
        .map(|t| &t.template)
}

/// File name without extension, e.g. when the original name is kept.
pub fn original_stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Alias, FolderTemplate, Rule};

    fn index(folders: &[&str]) -> FolderIndex {
        let all: Vec<String> = folders.iter().map(|s| s.to_string()).collect();
        FolderIndex { offered: all.clone(), all }
    }

    fn folders() -> FolderIndex {
        index(&[
            "08_Steuern",
            "02_Haus",
            "02_Haus/_Wasser_Abfall",
            "01_Versicherung/MuV_Versicherung",
            "Gehalt/2025_Erika",
            "Gehalt/2024_Erika",
        ])
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 10).unwrap()
    }

    fn date(y: i32, m: u32, d: u32) -> Option<DocDate> {
        NaiveDate::from_ymd_opt(y, m, d).map(DocDate::full)
    }

    fn cfg() -> Config {
        Config {
            archive_root: std::env::temp_dir().join("scrinium-does-not-exist").display().to_string(),
            persons: vec![
                Alias { name: "Max".into(), aliases: vec!["Maximilian".into()] },
                Alias { name: "Erika".into(), aliases: vec![] },
            ],
            senders: vec![Alias { name: "MuV".into(), aliases: vec!["M+V".into()] }],
            ..Default::default()
        }
    }

    #[test]
    fn page_without_text_layer_uses_ocr() {
        let scan = "Finanzamt Ulm\nBescheid für 2025 über Einkommensteuer\nUlm, 12.03.2026";
        // Scanned page without a text layer: the recognised text.
        assert_eq!(page_text("", Some(scan)), (scan.to_string(), true));
        // Only a closing phrase in the text layer; OCR finds the whole letter.
        assert_eq!(page_text("Mit freundlichen Grüßen", Some(scan)).0, scan);
        // If OCR finds nothing better, the short text layer stays.
        assert_eq!(page_text("Seite 2 von 2", Some("")), ("Seite 2 von 2".to_string(), false));
        assert_eq!(page_text("Seite 2 von 2", None), ("Seite 2 von 2".to_string(), false));
        // A broken text layer never counts, even when OCR fails.
        let garbled = "\u{e01a}\u{e01b}\u{e01c} ".repeat(30);
        assert_eq!(page_text(&garbled, None), (String::new(), false));
        assert_eq!(page_text(&garbled, Some(scan)), (scan.to_string(), true));
    }

    #[test]
    fn negated_extra_does_not_count() {
        let fine_print = "Diese Abrechnung wurde maschinell erstellt und wird nicht unterschrieben.";
        assert!(!extra_in_text("unterschrieben", fine_print));
        assert!(!extra_in_text("Kopie", "Ausfertigung ohne Kopie"));
        assert!(extra_in_text("unterschrieben", "Vertrag, von beiden Seiten unterschrieben am 03.04.2026"));
        assert!(extra_in_text("Anlage-1", "siehe ANLAGE 1 zum Vertrag"));
        assert!(!extra_in_text("Kopie", "Fotokopie"), "whole words only");
        assert!(!extra_in_text("", fine_print));
    }

    #[test]
    fn llm_answer_is_validated_against_structure() {
        let cfg = cfg();
        let text = "M+V Allgemeine Versicherung AG\nBeitragsrechnung\nDatum: 01.02.2024";
        let f = folders();
        let ctx = MergeContext { cfg: &cfg, folders: &f, text, today: today() };
        let mut s = heuristic_suggestion(&cfg, text, "scan", &f.offered, today());
        assert_eq!(s.doctype, "Beitragsrechnung");
        let mut warnings = Vec::new();
        let a = LlmAnalysis {
            target_folder: "Ausgedacht/Neu".into(),
            sender: "M+V Allgemeine Versicherung AG".into(),
            doctype: "Beitragsrechnungen".into(),
            person: "Maximilian Muster".into(),
            extra: "UNTERSCHRIEBEN".into(),
            ..Default::default()
        };
        merge_llm(&mut s, &a, "m", &ctx, &mut warnings);
        assert_eq!(s.sender, "MuV", "short name");
        assert_eq!(s.doctype, "Beitragsrechnung");
        assert_eq!(s.person, "Max");
        assert_eq!(s.extra, "", "„unterschrieben“ is not in the text");
        assert_eq!(s.origin.sender, Origin::Ai);
        assert_eq!(s.origin.doctype, Origin::Ai, "the AI confirms the type");
        assert_eq!(s.origin.extra, Origin::None);
        assert!(!s.confidence_from_ai, "no confidence from the AI");
        let copy = format!("{text}\nKOPIE");
        let ctx_copy = MergeContext { cfg: &cfg, folders: &f, text: &copy, today: today() };
        let mut c = s.clone();
        merge_llm(
            &mut c,
            &LlmAnalysis { extra: "kopie".into(), ..Default::default() },
            "m",
            &ctx_copy,
            &mut Vec::new(),
        );
        assert_eq!(c.extra, "Kopie");
        // No matching type in the list: the AI's own term stays instead of falling back to the closest one.
        let mut m = s.clone();
        let own = LlmAnalysis {
            doctype: llm::DOCTYPE_OTHER.into(),
            doctype_new: "meldebescheinigung".into(),
            ..Default::default()
        };
        merge_llm(&mut m, &own, "m", &ctx, &mut Vec::new());
        assert_eq!((m.doctype.as_str(), m.origin.doctype), ("Meldebescheinigung", Origin::Ai));
        assert!(extra_in_text("Anlage-1", "siehe Anlage 1") && !extra_in_text("sw", "Swisscom Rechnung"));
        assert_eq!(warnings.len(), 1, "an unknown folder is not taken");

        let a = LlmAnalysis { target_folder: "02_haus/_wasser_abfall".into(), ..Default::default() };
        merge_llm(&mut s, &a, "m", &ctx, &mut warnings);
        assert_eq!(s.folder, "02_Haus/_Wasser_Abfall");
    }

    #[test]
    fn llm_date_must_match_text() {
        let letter = "Musterstadt, 18.07.2024\nZeitraum 01.01.2023 bis 31.12.2023";
        assert_eq!(
            reconcile_date(date(2024, 7, 18), date(2023, 7, 18), letter, today()),
            date(2024, 7, 18),
            "made-up year"
        );
        assert_eq!(reconcile_date(date(2024, 7, 18), date(2023, 12, 31), letter, today()), date(2024, 7, 18), "period");
        let two = "Ihr Schreiben vom 01.02.2024\nBerlin, den 05.03.2024";
        assert_eq!(reconcile_date(date(2024, 2, 1), date(2024, 3, 5), two, today()), date(2024, 3, 5));
        assert_eq!(reconcile_date(date(2024, 3, 5), date(2024, 2, 1), two, today()), date(2024, 3, 5), "letter date");
        // Several letter dates: the newest, even against the AI.
        let misread = "Kontostand am 07.06.21)25\nMusterstadt, den 15.09.2025\nDatum 14.09.2025";
        assert_eq!(reconcile_date(date(2025, 9, 15), date(2025, 9, 14), misread, today()), date(2025, 9, 15));
        // Without a letter date, the AI decides among the dates of the clearest rank …
        let marked = "Rechnung vom 02.06.2026\nLieferung am 05.06.2026";
        assert_eq!(reconcile_date(date(2026, 6, 5), date(2026, 6, 2), marked, today()), date(2026, 6, 2));
        // … but it does not take a deadline when there is another date.
        let price = "Eine gute Nachricht. 14. November 2025\nzum 1. Januar 2026 senken wir den Preis";
        assert_eq!(reconcile_date(date(2025, 11, 14), date(2026, 1, 1), price, today()), date(2025, 11, 14));
        assert_eq!(reconcile_date(None, date(2022, 1, 1), "kein Datum", today()), date(2022, 1, 1));
        assert_eq!(reconcile_date(date(2024, 1, 1), date(2022, 1, 1), "", today()), date(2022, 1, 1), "image only");
        assert_eq!(reconcile_date(date(2024, 1, 1), None, "", today()), date(2024, 1, 1));
        assert_eq!(
            reconcile_date(None, DocDate::year_month(2016, 1), "Januar 2016", today()),
            DocDate::year_month(2016, 1)
        );
        assert_eq!(
            reconcile_date(date(2016, 1, 15), DocDate::year_month(2016, 1), "15.01.2016", today()),
            date(2016, 1, 15)
        );
        // The only full date is in the fine print on the back; it does not match February 2025.
        let payslip = "Mitteilung über die Bezüge FEBRUAR 2025\n…\n(Urteil des BGH vom 6. Mai 1997 - XI ZR 208/96)";
        assert_eq!(
            reconcile_date(date(1997, 5, 6), DocDate::year_month(2025, 2), payslip, today()),
            DocDate::year_month(2025, 2)
        );
        let year = Some(DocDate { year: 2025, month: None, day: None });
        assert_eq!(reconcile_date(date(1997, 5, 6), year, payslip, today()), year);
        assert_eq!(reconcile_date(date(2025, 2, 27), year, payslip, today()), date(2025, 2, 27));
    }

    #[test]
    fn new_folders_only_when_allowed() {
        let cfg = Config { allow_new_folders: true, ..cfg() };
        let f = folders();
        let ctx = MergeContext { cfg: &cfg, folders: &f, text: "", today: today() };
        let mut s = Suggestion::default();
        let a = LlmAnalysis { target_folder: "03_Auto/Werkstatt".into(), ..Default::default() };
        merge_llm(&mut s, &a, "m", &ctx, &mut Vec::new());
        assert_eq!(s.folder, "03_Auto/Werkstatt");
        let a = LlmAnalysis { target_folder: "../../etc".into(), ..Default::default() };
        merge_llm(&mut s, &a, "m", &ctx, &mut Vec::new());
        assert_eq!(s.folder, "03_Auto/Werkstatt");
    }

    #[test]
    fn rules_override_folder_and_year_folders_follow() {
        let cfg = Config {
            rules: vec![
                Rule {
                    keywords: vec!["Gehaltsmitteilung".into()],
                    folder: "Gehalt/{year}_Erika".into(),
                    person: Some("Erika".into()),
                    ..Default::default()
                },
                Rule {
                    keywords: vec!["Steuerfestsetzung".into()],
                    folder: "08_Steuern".into(),
                    doctype: Some("Steuerbescheid".into()),
                    ..Default::default()
                },
            ],
            ..cfg()
        };
        let mut s = Suggestion {
            date: date(2026, 2, 18),
            subject: "2026-02".into(),
            folder: "02_Haus".into(),
            ..Default::default()
        };
        finalize(&mut s, &cfg, &folders(), "Gehaltsmitteilung 02/2026", today());
        assert_eq!(s.folder, "Gehalt/2026_Erika", "rule with reference year, folder gets created");
        assert_eq!(s.person, "Erika");
        assert!(s.rule.is_some());

        let mut s = Suggestion { date: date(2026, 9, 25), subject: "2025".into(), ..Default::default() };
        finalize(&mut s, &cfg, &folders(), "Steuerfestsetzung", today());
        assert_eq!((s.folder.as_str(), s.doctype.as_str()), ("08_Steuern", "Steuerbescheid"));

        // Without a rule: the AI's folder from the previous year becomes the document's year folder.
        let mut s = Suggestion { date: date(2026, 1, 21), folder: "Gehalt/2025_Erika".into(), ..Default::default() };
        finalize(&mut s, &Config { rules: vec![], ..cfg.clone() }, &folders(), "", today());
        assert_eq!(s.folder, "Gehalt/2026_Erika");
        assert_eq!(s.folder_notes, vec!["Jahresordner für 2026"]);

        let mut s = Suggestion::default();
        finalize(&mut s, &cfg, &folders(), "", today());
        assert_eq!(s.folder, "00_Eingang", "unclear documents go to the inbox");
    }

    #[test]
    fn prompt_contains_structure_notes_and_text() {
        let mut cfg = cfg();
        cfg.folder_notes.insert("02_Haus/_Wasser_Abfall".into(), "Müll- und Wassergebühren".into());
        cfg.llm.instructions = "Gebühren gehören zum Haus.".into();
        let f = folders();
        let p = Prompt { cfg: &cfg, folders: &f.offered, text: "Hallo Welt", filename: "Scan.pdf", kind: DocKind::Pdf };
        let u = user_prompt(&p, 6000);
        assert!(!u.contains("02_Haus/_Wasser_Abfall") && u.contains("Hallo Welt"), "steps: no folder list");
        assert!(u.contains("Ursprünglicher Dateiname: Scan.pdf"));
        let empty = Prompt { text: "", kind: DocKind::Other, ..p };
        assert!(user_prompt(&empty, 6000).contains("Dateinamen"));
        let sys = system_prompt(&cfg);
        assert!(sys.contains("Max, Erika") && sys.contains("Gebühren gehören zum Haus."));
        assert!(!sys.contains("target_folder"), "folder choice in steps: no target folder in the first step");

        // Folder choice in one step: folder list with hints in the prompt.
        let mut one_step = cfg.clone();
        one_step.llm.folder_steps = false;
        let p = Prompt { cfg: &one_step, ..p };
        let u = user_prompt(&p, 6000);
        assert!(u.contains("- 02_Haus/_Wasser_Abfall — Müll- und Wassergebühren") && u.contains("Hallo Welt"));
        assert!(system_prompt(&one_step).contains("target_folder"));
    }

    #[test]
    fn folder_steps_offer_areas_with_children() {
        let mut cfg = cfg();
        cfg.folder_notes.insert("02_Haus".into(), "Haus und Grundstück".into());
        cfg.folder_notes.insert("02_Haus/_Wasser_Abfall".into(), "Müll und Wasser".into());
        let f = folders();
        let areas = child_options(&cfg, &f.offered, None);
        let names: Vec<&str> = areas.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(names, ["08_Steuern", "02_Haus", "01_Versicherung", "Gehalt"]);
        let (_, hint) = &areas[1];
        assert_eq!(hint, "Haus und Grundstück (Unterordner: _Wasser_Abfall: Müll und Wasser)");
        assert_eq!(child_options(&cfg, &f.offered, Some("02_Haus"))[0].0, "02_Haus/_Wasser_Abfall");

        // Year folders: with one series the year folder logic picks; several series are offered once each.
        let years = index(&[
            "Bank",
            "Bank/2025",
            "Bank/2026",
            "Bank/Depot",
            "Bank/202x",
            "Bank/Konto_2026_123456789",
            "Gehalt/2025_Erika",
            "Gehalt/2026_Erika",
            "Gehalt/2026_Max",
        ]);
        let bank: Vec<String> = child_options(&cfg, &years.offered, Some("Bank")).into_iter().map(|o| o.0).collect();
        assert_eq!(bank, ["Bank/Depot", "Bank/2026", "Bank/Konto_2026_123456789"], "two series: years and account");
        let pay = child_options(&cfg, &years.offered, Some("Gehalt"));
        let pay_names: Vec<&str> = pay.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(pay_names, ["Gehalt/2026_Erika", "Gehalt/2026_Max"]);
        assert!(pay[0].1.starts_with("Jahresordner"));
        assert!(archive::is_decade_bundle("202x") && archive::is_decade_bundle("201x_Depot"));
        assert!(!archive::is_decade_bundle("2026") && !archive::is_decade_bundle("20xx"));
        assert!(describe_prompt(&cfg).contains("Max (auch Maximilian); Erika"));
        let family = index(&["Familie", "Familie/Max", "Familie/Erika", "Familie/Behoerden", "Schule/Zeugnisse_Max"]);
        assert_eq!(folder_hint(&cfg, &family.offered, "Familie"), "(Unterordner: Behoerden; je Person einer)");
        assert_eq!(folder_hint(&cfg, &family.offered, "Schule"), "(Unterordner: je Person einer)");
        assert_eq!(
            naming::canonical_names("Zeugnis von Maximilian Muster, Maxi-Cosi.", &cfg.persons),
            "Zeugnis von Max Muster, Maxi-Cosi."
        );
        assert!(!fields_prompt(&cfg, false).contains("summary:") && system_prompt(&cfg).contains("summary:"));
    }

    #[test]
    fn folder_words_decide_or_narrow_the_choice() {
        let mut cfg = cfg();
        let options = |paths: &[&str]| -> Vec<(String, String)> {
            paths.iter().map(|p| (p.to_string(), String::new())).collect()
        };
        let tree = index(&[
            "01_Versicherung",
            "01_Versicherung/Haftpflicht",
            "01_Versicherung/Krankenkasse",
            "02_Haus",
            "Bank",
            "Gehalt",
            "Gehalt/2026_Erika",
            "Gehalt/2026_Max",
        ]);
        let f = &tree.offered;
        let level1 = options(&["01_Versicherung", "02_Haus", "Bank"]);
        let hay = |s: &str| archive::normalize(s);
        assert_eq!(apply_folder_words(&cfg, f, level1.clone(), &hay("Haftpflicht")), WordChoice::Ask(level1.clone()));

        cfg.folder_words.insert("01_Versicherung".into(), "Versicherung, Police, -Kontoauszug".into());
        cfg.folder_words.insert("02_Haus".into(), "Grundsteuer & Musterstadt".into());
        cfg.folder_words.insert("bank".into(), "Kontoauszug".into());
        assert_eq!(
            apply_folder_words(&cfg, f, level1.clone(), &hay("Ihre Police zur Haftpflicht-Versicherung")),
            WordChoice::Decided("01_Versicherung".into(), vec!["Versicherung".into(), "Police".into()])
        );
        // Negative word: the bank statement with an insurance premium belongs to the bank.
        assert_eq!(
            apply_folder_words(&cfg, f, level1.clone(), &hay("Kontoauszug: Beitrag Versicherung")),
            WordChoice::Decided("Bank".into(), vec!["Kontoauszug".into()])
        );
        // A negative word does not remove the folder from the options; the AI decides as without keywords.
        cfg.folder_words.insert("02_Haus".into(), "Grundsteuer & Musterstadt, -Familienzuschlag".into());
        assert_eq!(
            apply_folder_words(&cfg, f, level1.clone(), &hay("Grundsteuer Musterstadt, Familienzuschlag")),
            WordChoice::Ask(level1.clone())
        );
        cfg.folder_words.insert("02_Haus".into(), "Grundsteuer & Musterstadt".into());
        // „&“ requires both words; without a match the AI chooses.
        assert_eq!(
            apply_folder_words(&cfg, f, level1.clone(), &hay("Grundsteuer Ulm")),
            WordChoice::Ask(level1.clone())
        );
        // Several matches: the AI chooses and sees the keywords.
        let WordChoice::Ask(both) =
            apply_folder_words(&cfg, f, level1.clone(), &hay("Police; Grundsteuer der Stadt Musterstadt"))
        else {
            panic!("several matches: the AI chooses");
        };
        assert_eq!(both[0].1, "im Dokument: Police");
        assert_eq!(both[1].1, "im Dokument: Grundsteuer & Musterstadt");
        assert_eq!(both[2].1, "");

        // A subfolder's keywords also count for the parent folder, except with a negative word of the subfolder.
        cfg.folder_words.insert("01_Versicherung/Krankenkasse".into(), "Muster Krankenkasse, -Lohnsteuer*".into());
        cfg.folder_words.insert("01_Versicherung/Gibts_nicht".into(), "Beitragsrechnung".into());
        assert_eq!(
            apply_folder_words(&cfg, f, level1.clone(), &hay("Muster Krankenkasse: neuer Zusatzbeitrag")),
            WordChoice::Decided("01_Versicherung".into(), vec!["Muster Krankenkasse".into()])
        );
        let pay_slip = hay("Lohnsteuerbescheinigung, Krankenkasse: Muster Krankenkasse");
        assert_eq!(apply_folder_words(&cfg, f, level1.clone(), &pay_slip), WordChoice::Ask(level1.clone()));
        let unknown = hay("Beitragsrechnung");
        assert_eq!(
            apply_folder_words(&cfg, f, level1, &unknown),
            WordChoice::Ask(options(&["01_Versicherung", "02_Haus", "Bank"]))
        );

        // Year folders: key with {year}; also counts for the parent folder.
        cfg.folder_words.insert("Gehalt/{year}_Max".into(), "Musterfirma".into());
        let pay = options(&["Gehalt/2026_Erika", "Gehalt/2026_Max"]);
        let slip = hay("Musterfirma GmbH, Entgeltabrechnung");
        assert_eq!(
            apply_folder_words(&cfg, f, pay, &slip),
            WordChoice::Decided("Gehalt/2026_Max".into(), vec!["Musterfirma".into()])
        );
        assert_eq!(
            apply_folder_words(&cfg, f, options(&["Bank", "Gehalt"]), &slip),
            WordChoice::Decided("Gehalt".into(), vec!["Musterfirma".into()])
        );
        assert!(words_folder_exists(f, "Gehalt/{year}_Max") && words_folder_exists(f, "{year}"));

        // Without AI: level by level, as long as exactly one folder matches.
        let (folder, why) = folder_by_words(&cfg, f, &slip);
        assert_eq!(folder, "Gehalt/2026_Max");
        assert_eq!(why, ["Gehalt: Stichwort „Musterfirma“", "2026_Max: Stichwort „Musterfirma“"]);
        let (folder, _) = folder_by_words(&cfg, f, &hay("Muster Krankenkasse"));
        assert_eq!(folder, "01_Versicherung/Krankenkasse");
        assert_eq!(folder_by_words(&cfg, f, &hay("Haftpflicht")).0, "", "no match: no folder");
        assert!(!words_folder_exists(f, "Lohn/{year}") && !words_folder_exists(f, "01_Versicherung/Gibts_nicht"));
    }

    #[test]
    fn filenames_use_folder_templates() {
        let mut cfg = cfg();
        cfg.folder_templates.push(FolderTemplate {
            folder: "06_Haushalt/Anleitungen_Handbücher".into(),
            template: "{sender}_{subject}_Anleitung".into(),
        });
        let f = naming::TemplateFields {
            date: date(2026, 5, 20),
            doctype: "Übersicht",
            sender: "Musterholz",
            subject: "MM100 Garderobe",
            ..Default::default()
        };
        assert_eq!(
            suggested_filename(&cfg, &f, "06_Haushalt/Anleitungen_Handbücher/Möbel"),
            "Musterholz_MM100-Garderobe_Anleitung"
        );
        assert_eq!(
            suggested_filename(&cfg, &f, "06_Haushalt/Rechnungen"),
            "260520_Übersicht_Musterholz_MM100-Garderobe"
        );
    }
}
