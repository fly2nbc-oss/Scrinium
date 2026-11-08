#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod about;
mod analysis;
mod app;
mod archive;
mod config;
mod doclang;
mod download;
mod folder_view;
mod form;
mod heuristics;
mod i18n;
mod ingest;
mod journal;
mod llm;
mod llm_ui;
mod local_llm;
mod naming;
mod ocr;
mod pages;
mod preprocess;
mod rotate;
mod scan;
mod scan_pdf;
mod schema;
mod settings;
mod setup;
mod tags;
mod tesseract;
mod theme;
#[cfg(windows)]
mod twain;
mod update;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use eframe::egui;

fn main() -> eframe::Result {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let cli = |f: &dyn Fn() -> i32| -> ! {
        attach_console();
        std::process::exit(f())
    };
    match args.first().and_then(|a| a.to_str()) {
        Some("--analyse" | "--analyze") => cli(&|| analyse_cli(&args[1..])),
        Some("--gpu") => cli(&gpu_cli),
        Some("--scanner") => cli(&|| {
            let devices = scan::find_devices();
            if devices.is_empty() {
                println!("No scanner found.");
            }
            for d in devices {
                println!("{:<40} {}", d.id, d.label);
            }
            0
        }),
        Some("--setup") => cli(&|| setup_cli(&args[1..])),
        Some("--report") => cli(&|| report_cli(&args[1..])),
        Some("--pdf") => cli(&|| pdf_cli(&args[1..])),
        Some("--download-model") => {
            let id = args.get(1).map(|a| a.to_string_lossy().into_owned()).unwrap_or_else(|| "auto".into());
            cli(&|| download_cli(&id))
        }
        Some("--help" | "-h") => cli(&|| {
            println!(
                "Scrinium {}\n\n  scrinium [FILES …]                   Window, files go straight into the queue\n  \
                 scrinium --analyze [--no-ai] FILES … Print the suggestion as JSON; nothing is moved\n  \
                 scrinium --setup [--json] TEXT       Settings from a description of the archive (TOML)\n  \
                 scrinium --report [FILE]             How often suggestions were kept or changed\n  \
                 scrinium --gpu                       Graphics card, memory and recommended model\n  \
                 scrinium --scanner                   Scanners found (TWAIN, WIA, SANE)\n  \
                 scrinium --pdf TARGET.pdf IMAGE …    Images as a searchable PDF (--mode bw|color|brochure|photo)\n  \
                 scrinium --download-model [auto|ID]  Download an AI model\n\n\
                 Configuration: {}",
                env!("CARGO_PKG_VERSION"),
                config::config_path().display()
            );
            0
        }),
        _ => {}
    }

    i18n::init(config::config_path().parent().unwrap_or(std::path::Path::new(".")));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(i18n::general::t().window_title)
            .with_app_id("scrinium")
            .with_icon(icon())
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([820.0, 560.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native("Scrinium", options, Box::new(move |cc| Ok(Box::new(app::ScriniumApp::new(cc, args)))))
}

/// Window and taskbar icon; `build.rs` embeds the icon of the exe.
fn icon() -> egui::IconData {
    // The PNG is built into the program; if it could not be read, the window would keep the default icon.
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).unwrap_or_default()
}

/// In release builds Scrinium is a GUI program. For the command line, attach to the calling console so that output
/// shows up in cmd and PowerShell.
fn attach_console() {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn AttachConsole(process_id: u32) -> i32;
        }
        // ATTACH_PARENT_PROCESS; fails harmlessly without a console (e.g. when output is redirected).
        unsafe {
            AttachConsole(u32::MAX);
        }
    }
}

/// `scrinium --report [FILE]`: summarize the decision log.
fn report_cli(args: &[PathBuf]) -> i32 {
    let path = args.first().cloned().unwrap_or_else(journal::path);
    if !path.exists() {
        println!("No decisions recorded yet ({} is missing).", path.display());
        return 0;
    }
    match journal::read(&path) {
        Ok((entries, broken)) => {
            println!("Decisions from {}\n", path.display());
            print!("{}", journal::report(&entries));
            match broken {
                0 => {}
                1 => println!("\n1 line could not be read."),
                n => println!("\n{n} lines could not be read."),
            }
            0
        }
        Err(e) => {
            eprintln!("{e:#}");
            2
        }
    }
}

/// `scrinium --gpu`: print the detected compute devices and the recommended model.
fn gpu_cli() -> i32 {
    let devices = match local_llm::detect_devices() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e:#}");
            return 2;
        }
    };
    println!("Detected devices:");
    for d in &devices {
        let free = if d.is_gpu() { format!(", {} free", download::format_bytes(d.memory_free)) } else { String::new() };
        println!("  [{}] {} – {}{free}", d.index, d.kind_label(), d.summary());
    }
    if !devices.iter().any(local_llm::Device::is_gpu) {
        println!("  (no GPU – Vulkan driver installed? Built with feature \"vulkan\"?)");
    }
    let rec = local_llm::recommend(&devices);
    println!("\nRecommended: {} ({})", rec.model.name, download::format_bytes(rec.model.total_size()));
    println!("  {}", rec.reason);
    for m in local_llm::CATALOG {
        let state = if m.is_installed() { "installed" } else { "not downloaded" };
        println!("  {:<12} {:>8}  {state}  – {}", m.id, download::format_bytes(m.total_size()), m.note);
    }
    0
}

/// `scrinium --pdf target.pdf image.jpg … [--mode bw|color|brochure|photo]`: prepare images like a scan (deskew,
/// color mode, text recognition) and save them as a searchable PDF. The resolution is estimated from the image
/// size as if the longer side were 29.7 cm long (DIN A4).
fn pdf_cli(args: &[PathBuf]) -> i32 {
    let usage = "usage: scrinium --pdf <target.pdf> <image> … [--mode bw|color|brochure|photo]";
    let mut preset = None;
    let mut paths = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a.as_os_str() == "--mode" {
            preset = match it.next().and_then(|v| v.to_str()) {
                Some("bw") => Some(config::ScanPreset::BlackWhite),
                Some("color") => Some(config::ScanPreset::Color),
                Some("brochure") => Some(config::ScanPreset::Brochure),
                Some("photo") => Some(config::ScanPreset::Photo),
                _ => {
                    eprintln!("{usage}");
                    return 2;
                }
            };
        } else {
            paths.push(a.clone());
        }
    }
    let Some((target, images)) = paths.split_first().filter(|(_, rest)| !rest.is_empty()) else {
        eprintln!("{usage}");
        return 2;
    };
    let cfg = config::Config::load_or_create(&config::config_path()).unwrap_or_default();
    let preset = preset.unwrap_or(cfg.scan.preset);
    let ocr = match ocr::Ocr::new(&cfg.ocr, &|s| eprintln!("{s}")) {
        Ok(o) => Some(o),
        Err(e) => {
            eprintln!("Without text recognition: {e:#}");
            None
        }
    };
    let mut pages = Vec::new();
    let mut notes = Vec::new();
    for path in images {
        let image = match image::open(path) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                return 1;
            }
        };
        let dpi = (image.width().max(image.height()) as f32 / (297.0 / 25.4)).clamp(72.0, 1200.0);
        let page = scan::ScannedPage { image, dpi };
        match scan_pdf::prepare_page(&page, preset, ocr.as_ref(), true, &mut notes) {
            Ok(p) => {
                let words: usize = p.lines.iter().map(Vec::len).sum();
                eprintln!("{}: {} lines, {words} words, {dpi:.0} dpi", path.display(), p.lines.len());
                pages.push(p);
            }
            Err(e) => {
                eprintln!("{}: {e:#}", path.display());
                return 1;
            }
        }
    }
    for note in notes {
        eprintln!("Note: {note}");
    }
    match scan_pdf::write(&pages, target) {
        Ok(()) => {
            eprintln!("{} saved ({}).", target.display(), preset.label());
            0
        }
        Err(e) => {
            eprintln!("Error: {e:#}");
            1
        }
    }
}

/// `scrinium --download-model [auto|qwen3-vl-2b|…]`
fn download_cli(id: &str) -> i32 {
    let spec = if id == "auto" {
        match local_llm::detect_devices() {
            Ok(d) => local_llm::recommend(&d).model,
            Err(e) => {
                eprintln!("{e:#}");
                return 2;
            }
        }
    } else {
        match local_llm::find_model(id) {
            Some(s) => s,
            None => {
                let ids: Vec<&str> = local_llm::CATALOG.iter().map(|m| m.id).collect();
                eprintln!("Unknown model \"{id}\". Available: auto, {}", ids.join(", "));
                return 2;
            }
        }
    };
    eprintln!("Downloading {} to {} …", spec.name, local_llm::models_dir().display());
    let cancel = AtomicBool::new(false);
    let mut last = String::new();
    let result = local_llm::download_model(
        spec,
        &mut |p| {
            let line = match p.fraction() {
                Some(f) => format!("{} ({:.0} %)", p.describe(), f * 100.0),
                None => p.describe(),
            };
            if line != last {
                eprint!("\r{line}    ");
                last = line;
            }
        },
        &cancel,
    );
    eprintln!();
    match result {
        Ok(()) => {
            eprintln!("{} is ready.", spec.name);
            0
        }
        Err(e) => {
            eprintln!("Error: {e:#}");
            1
        }
    }
}

/// `scrinium --setup [--json] <description.md>`: the AI derives settings from the description of the filing
/// system. Prints the resulting configuration as TOML (with `--json` the raw suggestion); nothing is saved.
fn setup_cli(args: &[PathBuf]) -> i32 {
    let json = args.iter().any(|a| a.as_os_str() == "--json");
    let Some(path) = args.iter().find(|a| a.as_os_str() != "--json") else {
        eprintln!("usage: scrinium --setup [--json] <description.md>");
        return 2;
    };
    let mut cfg = match config::Config::load_or_create(&config::config_path()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e:#}");
            return 2;
        }
    };
    let description = match std::fs::read(path) {
        Ok(b) => ingest::decode_text(&b),
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return 2;
        }
    };
    let folders = archive::FolderIndex::scan(&cfg.archive_root(), cfg.scan_depth, &cfg.exclude, &cfg.collapse);
    let (tx, _, rx) = analysis::spawn_worker(|| {});
    let job =
        analysis::SetupJob { config: cfg.clone(), description, folders: folders.clone(), cancel: Default::default() };
    if tx.send(analysis::Work::Setup(job)).is_err() {
        eprintln!("analysis thread is not running");
        return 1;
    }
    drop(tx);
    for event in rx {
        match event {
            analysis::Event::SetupProgress(m) => eprintln!("{m}"),
            analysis::Event::SetupCancelled => return 1,
            analysis::Event::SetupDone(Err(e)) => {
                eprintln!("Error: {e}");
                return 1;
            }
            analysis::Event::SetupDone(Ok(p)) => {
                if json {
                    println!("{}", serde_json::to_string_pretty(&p).unwrap_or_default());
                    return 0;
                }
                for note in setup::apply(&p, &mut cfg, &folders, setup::Sections::default()) {
                    eprintln!("Note: {note}");
                }
                match toml::to_string_pretty(&cfg) {
                    Ok(t) => println!("{t}"),
                    Err(e) => {
                        eprintln!("{e}");
                        return 1;
                    }
                }
                return 0;
            }
            _ => {}
        }
    }
    1
}

/// `scrinium --analyze [--no-ai] <file> …`: print the suggestion without the window (nothing is moved).
/// One JSON line per document.
fn analyse_cli(args: &[PathBuf]) -> i32 {
    let mut cfg = match config::Config::load_or_create(&config::config_path()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e:#}");
            return 2;
        }
    };
    let without_ai = args.iter().any(|a| a.as_os_str() == "--no-ai");
    if without_ai {
        cfg.llm.enabled = false;
    }
    let files: Vec<PathBuf> = args.iter().filter(|a| a.as_os_str() != "--no-ai").cloned().collect();
    let folders = archive::FolderIndex::scan(&cfg.archive_root(), cfg.scan_depth, &cfg.exclude, &cfg.collapse);
    let (tx, _, rx) = analysis::spawn_worker(|| {});
    for (i, path) in files.iter().enumerate() {
        let job = analysis::Job { id: i as u64, path: path.clone(), config: cfg.clone(), folders: folders.clone() };
        if tx.send(analysis::Work::Analyse(job)).is_err() {
            eprintln!("analysis thread is not running");
            return 1;
        }
    }
    drop(tx);
    let mut code = 0;
    for event in rx {
        match event {
            analysis::Event::Progress { id, message } => eprintln!("[{}] {message}", files[id as usize].display()),
            analysis::Event::Failed { id, error } => {
                eprintln!("[{}] error: {error}", files[id as usize].display());
                code = 1;
            }
            analysis::Event::SetupProgress(_)
            | analysis::Event::SetupDone(_)
            | analysis::Event::SetupCancelled
            | analysis::Event::Preview { .. } => {}
            analysis::Event::Done { id, analysis } => {
                let s = &analysis.suggestion;
                let path = &files[id as usize];
                let filename = if s.keep_name {
                    analysis::original_stem(path)
                } else {
                    analysis::suggested_filename(
                        &cfg,
                        &naming::TemplateFields {
                            date: s.date,
                            doctype: &s.doctype,
                            sender: &s.sender,
                            subject: &s.subject,
                            person: &s.person,
                            extra: &s.extra,
                        },
                        &s.folder,
                    )
                };
                let out = serde_json::json!({
                    "file": path,
                    "kind": analysis.kind.label(),
                    "date": s.date.map(naming::DocDate::display),
                    "doctype": s.doctype,
                    "sender": s.sender,
                    "subject": s.subject,
                    "person": s.person,
                    "extra": s.extra,
                    "summary": s.summary,
                    "folder": s.folder,
                    "folder_notes": s.folder_notes,
                    "folder_why": s.folder_why,
                    "folder_exists": folders.exists(&s.folder),
                    "filename": filename,
                    "source": s.source,
                    "rule": s.rule,
                    "confidence": s.confidence,
                    "ocr": analysis.ocr_engine,
                    "corrections": analysis.corrections,
                    "warnings": analysis.warnings,
                    "seconds": analysis.duration.as_secs_f32(),
                    "ai_time": analysis.ai_stats,
                    "text": analysis.text.chars().take(4000).collect::<String>(),
                });
                println!("{}", serde_json::to_string(&out).unwrap_or_default());
            }
        }
    }
    code
}
