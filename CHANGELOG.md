# Changelog

All notable changes to Scrinium are documented here.
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versions follow [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

## [1.0.1] – 2026-11-09

Dependency updates: llama-cpp-2 0.1.160, lopdf 0.46, libloading 0.9, serde_json, sha2 and winresource; cargo-deny installer in CI 2.89.0.

## [1.0.0] – 2026-11-09

First public release. Scrinium files scanned and digital documents into your existing folder archive with an AI that runs on your own computer.

### Features

- **Drop, check, file:** drag in scans, PDFs, images, Office files, text, HTML, RTF or e-mails. Scrinium suggests date, document type, sender, subject, person, target folder and file name; you check the suggestion and file the document with one click (Ctrl+Enter). Undo puts it back.
- **Local AI:** Qwen3-VL through llama.cpp inside the program, on the GPU via Vulkan (AMD, NVIDIA, Intel) or on the CPU. The app recommends and downloads a model that fits the hardware. An external OpenAI-compatible server (LM Studio, Ollama) is optional.
- **Your archive, your folders:** the AI chooses only existing folders, level by level, guided by short folder descriptions. Keywords (*Recognised by*) decide without the AI when exactly one folder matches; a negative word (`-word`) cancels the hits of its folder. Year folders are handled automatically. New folders only when you allow them.
- **File names from parts:** date, document type, sender, subject, person and extra, in the order you drag them; separator, date format and character rules are configurable. Senders get short names, people are recognised by name and other spellings.
- **Special rules** file recurring documents without the AI.
- **Scanning:** TWAIN with the manufacturer's dialog, otherwise WIA on Windows and SANE on Linux. Pages are straightened and saved as a searchable PDF with a text layer; *Scan as* chooses *B/W document*, *Colour document*, *Brochure* or *Photo*.
- **Text recognition:** Tesseract for scans and images (German, English, French, Spanish and Italian with one click, more by their Tesseract code); crooked pages are turned upright and straightened. Pages you rotate in the preview are saved rotated when filed (PDF, JPEG, PNG).
- **Windows Search:** title and tags are written into PDFs, Word, Excel and PowerPoint files and, on Windows, images.
- **Set up with AI:** describe your filing system in a few sentences and get a checked proposal for the configuration.
- **Decision log:** `decisions.jsonl` records suggestion and final value per document; `scrinium --report` shows which suggestions are often changed.
- **Command line:** `--analyze` prints a suggestion as JSON without moving anything; also `--setup`, `--report`, `--gpu`, `--scanner`, `--pdf`, `--download-model` and `--help`.
- **Interface** in English, German, French, Spanish and Italian; light and dark appearance; keyboard shortcuts for filing, paging and switching documents. The detection without AI (dates, document types, senders) is built for German-language documents.
- **Privacy:** documents never leave the computer unless you set up an external AI server. The update check runs only after you allow it, at most once a day.
- **Packages:** Windows installer and portable ZIP, Linux `.deb`, AppImage and `.tar.gz`.
