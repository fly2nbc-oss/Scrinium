# Third-party components

Scrinium's own code is licensed under Apache-2.0 (see [LICENSE](./LICENSE)). It uses or contains the following work by others.

## Contained in the program file

| Component | Used for | License |
|---|---|---|
| [llama.cpp](https://github.com/ggml-org/llama.cpp) and ggml, through [`llama-cpp-2`](https://github.com/utilityai/llama-cpp-rs) | built-in AI, Vulkan backend, image projector (mtmd) | MIT, © The ggml authors; the Rust bindings MIT or Apache-2.0 |
| [Lucide](https://lucide.dev) (`assets/lucide/lucide-subset.ttf`, subset of `lucide-static@0.460.0`) | interface icons | ISC, see [`assets/lucide/LICENSE`](./assets/lucide/LICENSE) |
| egui fonts (`epaint_default_fonts`): Ubuntu Light, Hack, Noto Emoji, emoji-icon-font | fallback fonts when the system has no suitable one | Ubuntu Font Licence 1.0, MIT/Bitstream Vera (Hack), SIL OFL 1.1 (Noto Emoji), MIT (emoji-icon-font) |
| Rust crates (egui/eframe, pdfium-render, lopdf, image, imageproc, ureq, zip, toml, serde, regex, chrono, sys-locale, libloading, sha2, windows …) | see `Cargo.lock` | mostly MIT or Apache-2.0 |

### Windows only: Tesseract

`scrinium.exe` contains `tesseract55.dll` (built with `packaging/tesseract`, release `tesseract-5.5.3-1`); Scrinium unpacks it into its cache on first use. The DLL contains these libraries; the Windows installer and the portable ZIP put their license texts into `licenses-tesseract/` in the program folder.

| Component | License |
|---|---|
| [Tesseract](https://github.com/tesseract-ocr/tesseract) 5.5.3 | Apache-2.0 |
| [Leptonica](http://www.leptonica.org) 1.87.0 | BSD-2-Clause |
| [zlib](https://zlib.net) 1.3.2 | Zlib |
| [libpng](http://www.libpng.org) 1.6.59 | libpng-2.0 |
| [libjpeg-turbo](https://libjpeg-turbo.org) 3.2.0 | IJG and BSD-3-Clause – *this software is based in part on the work of the Independent JPEG Group* |
| [libtiff](http://www.libtiff.org) 4.7.2 | libtiff (MIT-like) |
| [libwebp](https://chromium.googlesource.com/webm/libwebp) 1.6.0 with libsharpyuv | BSD-3-Clause and patent grant by Google |
| [giflib](https://giflib.sourceforge.net) 6.1.3 | MIT |
| [OpenJPEG](https://www.openjpeg.org) 2.5.4 | BSD-2-Clause |

### Windows packages only: Visual C++ runtime

The installer and the portable ZIP put `msvcp140.dll`, `vcruntime140.dll`, `vcruntime140_1.dll` and `vcomp140.dll` (OpenMP for the AI on the processor) next to `scrinium.exe`, so no Visual C++ Redistributable is needed. They come from the Redist folder of Visual Studio, where they are released as “Distributable Code” for distribution with applications (Microsoft Visual Studio license terms).

## Bundled in the Linux AppImage

Libraries from Ubuntu 24.04 that are not present on every system (e.g. the OpenMP runtime `libgomp`), under their own licenses. The list with package versions is in the AppImage under `usr/share/doc/scrinium/bundled-libraries.txt`, the source code in Ubuntu's archive. The Vulkan loader and the graphics drivers always come from the system.

## Loaded at run time (not distributed)

| Component | When | License |
|---|---|---|
| [Qwen3-VL-2B/4B/8B-Instruct](https://huggingface.co/Qwen) (GGUF, Q4_K_M, with mmproj) from Hugging Face | when you click *Download* under *Settings › AI* (1.5 to 5.4 GB) | Apache-2.0, © Alibaba Cloud (Qwen team) |
| [PDFium](https://pdfium.googlesource.com/pdfium/), builds from [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) | with the first PDF (about 4 MB) | BSD-3-Clause and Apache-2.0 (PDFium and its dependencies); the license texts come with the download |
| Language data for Tesseract from [tessdata_fast](https://github.com/tesseract-ocr/tessdata_fast) or [tessdata_best](https://github.com/tesseract-ocr/tessdata_best) | per chosen language with the first text recognition (fast 1–4 MB, best 4–15 MB); deu, eng, fra, spa, ita checked with SHA-256 | Apache-2.0 |

The files go into Scrinium's cache folder and are not modified.

## Used from the system

- **Vulkan** – comes with the graphics driver.
- **Segoe UI** (Windows) or DejaVu Sans / Noto Sans (Linux) as the interface font – read from the system at start, not distributed.
- **Tesseract on Linux** (Apache-2.0) – `libtesseract.so.5` from the package `libtesseract5`, a dependency of the `.deb`; the AppImage uses the installed library and otherwise shows the command to install it. If all chosen languages are available in the `tesseract-ocr-…` packages and the accuracy is *Fast*, Scrinium uses their language data.
- **LM Studio / Ollama** (optional) – external AI server, used through its OpenAI-compatible interface.
