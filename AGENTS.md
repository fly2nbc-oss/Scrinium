# AGENTS.md

Scrinium (Latin for "writing case, archive") files documents into an existing folder archive with a **local AI**: drop in a scan, PDF, image, Office file or e-mail, check the suggested date, type, sender, subject, target folder and file name, and file it. Rust with **egui/eframe (glow)**; the AI is llama.cpp inside the program. Windows and Linux. This file is for AI agents (Claude Code, Codex, Cursor, Copilot …) and equally readable for people; `CLAUDE.md` only imports it.

## Standards

This project follows the latest version of two documents in the parent folder (outside this repo): `repo_standard_v*.md` (structure, docs, CI, releases) and `ui_design_system_v*.md` (user interface). Read them before changing those areas; where this file says otherwise, this file wins.

## Rules

- Code, comments and docs are English. The UI has five languages (`src/i18n/`).
- Text for the language model (prompts, option lists, schema descriptions) stays German on purpose. Everything that depends on the language of the documents (dates, doctypes, senders, legal forms, default doctypes) lives in `src/doclang/` – one file per document language, independent of the UI language in `src/i18n/`.
- The program does not abort: `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` in `main.rs`; tests may (`clippy.toml`). Fixed regex patterns go through `heuristics::code_regex`.
- Before every commit: format, Clippy without warnings, tests (commands below).
- Push, tags and releases only when the maintainer says so; releases only after the maintainer has tested the local build.
- No real documents, names, addresses, account or contract numbers in tests, examples, screenshots and commits – not even taken from file names. Use fictional values (Max Mustermann, Musterstadt, `DE00 1234 …`).
- **No simulated keystrokes or mouse clicks** on the user's machine while they are working. Never stop test copies of the exe by process name (`taskkill /IM scrinium.exe` would also hit the user's own instance); use the PID or path.

## Commands

```bash
pwsh scripts/cargo-windows.ps1 build --release                         # Windows (short path S:, CMake, LLVM)
pwsh scripts/cargo-windows.ps1 clippy --release --all-targets --locked -- -D warnings
pwsh scripts/cargo-windows.ps1 test --release --locked
cargo fmt --all -- --check
cargo deny --locked check                                              # vulnerabilities, licenses, sources (deny.toml)
pwsh scripts/fetch-tesseract.ps1                                       # Tesseract DLL for the Windows build
SCRINIUM_CONFIG=<test.toml> target/release/scrinium --analyze [--no-ai] <files>   # suggestion as JSON, moves nothing
```

On Linux the same cargo commands directly. `rust-toolchain.toml` pins the Rust version. Packages (as CI builds them after the release build): `pwsh packaging/windows/build.ps1` → `dist\scrinium_<v>_x64-setup.exe`, `dist\scrinium_<v>_x64-portable.zip`; `bash packaging/linux/build.sh` → `.deb` and AppImage (Ubuntu 24.04).

## Product decisions (settled – build on them, do not propose alternatives)

| Decision | Why |
|---|---|
| AI inside the program (llama.cpp, Qwen3-VL), no service needed | Nothing leaves the computer. An external server (LM Studio, Ollama) is only an option. |
| The AI chooses **only existing folders**, level by level, guided by folder descriptions | The archive belongs to the user; new folders only if they switch it on. |
| **Keywords** (*Recognised by*, `[folder_words]`) decide without the AI when exactly one folder matches; a negative word (`-word`) cancels the hits of its folder but does not exclude it | Clear cases fast and safe; borderline cases stay with the AI. |
| Flash attention off under Vulkan | Measured 4–5× slower on AMD RDNA2. |
| Tesseract on Windows as one DLL embedded in the exe, on Linux the system library | One program without extras; the `tesseract-*` release is never "latest", otherwise it confuses the update check. |
| Update check only after consent, at most once a day | R7 of the repo standard (no network traffic without consent). |
| Status bar 40 instead of 28 px | It carries icon buttons with a 32 px hit area. |
| `spacing.interact_size.y = 32` globally | Almost every row carries a control; the uniform height is intended. |
| Border rule: `line` globally, `line_strong` only in `theme::field`, `check`, `radio_value` | Otherwise every selected row of an egui selectable list would get a strong border. |
| Dialog cards without × at the top right; lists and popups without `scrim-light` | The cards are small, Esc and a click outside close them; egui does not dim behind popups. |
| Cross on the parts of the file name 20 × 24 px | The parts sit close together. |
| Tab is not intercepted | In the form and the settings, Tab moves from field to field. |
| Scanning without a question; "Scan as [type]" sits in its own row below "Scan" and is saved at once | Every scan becomes a PDF; several pages via the feeder or the scanner dialog. |

## Layout

```
src/
  main.rs        start, window, --analyze, --setup, --report, --gpu, --scanner, --pdf, --download-model, --help
  config.rs      config.toml: keys, defaults, load and save (backup of a commented file), paths
  app.rs         main window: queue, detail view, status bar, undo
  form.rs        suggestion as a table, target with folder search and file list, file name, preview
  settings.rs    settings: archive, file names, special rules, AI, general; validation before saving
  schema.rs      file name structure: drag parts, separator, template as text
  scan.rs        scanning: TWAIN, otherwise WIA (Windows), SANE (Linux); find scanners, fetch pages
  scan_pdf.rs    prepare scans (straighten, colour mode) and save them as a searchable PDF with a text layer
  twain.rs       TWAIN: TWAINDSM.dll, the manufacturer's dialog, images (DIB) from the feeder
  about.rs       "General" tab (about, language, appearance, updates, shortcuts) and "Scan" tab
  i18n/          UI texts in five languages, one file per area
  update.rs      check for a new version (only when allowed, at most once a day)
  folder_view.rs folder overview: description for the AI, keywords ("Recognised by")
  setup.rs       set up with AI: description → checked proposal for the configuration
  tags.rs        properties for Windows Search (PDF, Office, images)
  theme.rs       design system: light/dark tokens, sizes, fonts, Lucide, components
  llm_ui.rs      settings of the built-in AI: graphics card, download models
  analysis.rs    pipeline in a background thread, reconcile AI ↔ text ↔ rules, year folders
  naming.rs      file names: date, character rules, people, senders, document types
  local_llm.rs   built-in AI (llama.cpp): GPU detection, model catalogue, image + text → JSON via grammar
  llm.rs         JSON schema of the answer, OpenAI-compatible client
  download.rs    downloads with resume and progress
  ingest.rs      PDF (PDFium in its own loader thread), images, Office, text, HTML, RTF, e-mail
  pages.rs       preview pages: analysed pages plus the rest, loaded while paging
  rotate.rs      save pages rotated on filing (PDF /Rotate as incremental update, JPEG, PNG)
  preprocess.rs  coarse rotation, deskewing, contrast
  ocr.rs         text recognition: load and check language data, text and word boxes (TSV)
  tesseract.rs   Tesseract via its C API, loaded at run time
  heuristics.rs  date, document type, sender, subject without AI; code_regex
  doclang/       document language (not UI language): German word lists and patterns in de.rs
  archive.rs     read folders, rules, keywords, safe move and undo
  journal.rs     decisions.jsonl: suggestion and filed values per document, --report
```

The UI uses the components from `theme.rs` (buttons, fields, checkboxes, segments, chips, selectable lists, dialog cards) instead of the egui defaults; tokens only, no hex values in the views.

## Tests

Tests live in their module (`#[cfg(test)]`) and need neither a model nor a GPU: date detection, heuristics, file names, year folders, rules and keywords, Office and text files, JSON schema, undo, safe paths, deskewing and rotation, AI client against a simulated server, resumed downloads. `config.rs` checks that `config.example.toml` parses.

## Release

1. Raise the version in `Cargo.toml` (the single source; *Settings › General › About Scrinium* shows it).
2. `CHANGELOG.md`: rename `[Unreleased]` to `## [X.Y.Z] – YYYY-MM-DD`, one sentence below it, a new empty `[Unreleased]` above. Visibly changed UI: renew the screenshot `screenshots/scrinium-hell.png`.
3. Commit, build locally; the maintainer tests the build.
4. Push when the maintainer says so. Wait for CI.
5. When the maintainer says so: `git tag -a vX.Y.Z -m "Scrinium X.Y.Z"`, push the tag. CI builds, tests the packages and creates the draft.
6. Check the draft, then `gh release edit vX.Y.Z --draft=false --latest`.

## Pitfalls

- **Windows build only via `scripts/cargo-windows.ps1`:** the Vulkan build of llama.cpp exceeds MSBuild's 260-character limit from a deep project path; the script builds from the short path `S:`. CMake remembers the path of the first build.
- Locally on Windows check only the release profile; the debug build of llama.cpp fails with Visual Studio 18 and CMake 4. CI covers debug on Linux.
- A running `scrinium.exe` locks `target/release/scrinium.exe`. Often it is the user's test instance: do not stop it, rename the file instead (Windows allows that), then build.
- Antivirus software with its own root CA breaks rustls with webpki roots: `ureq` therefore uses `native-certs`.
- egui 0.31: `on_hover_text` shows only on enabled widgets, `on_disabled_hover_text` only on disabled ones. A widget that can be both needs both. `Sense::click()` takes keyboard focus; click areas use `egui::Sense::CLICK` without `FOCUSABLE` (R7); only checkboxes and fields stay focusable.
- A push that changes only `*.md` or `screenshots/` starts no CI; tags always build.
- The *Vulkan SDK* step (Chocolatey) sometimes fails temporarily: `gh run rerun <id> --failed`.
