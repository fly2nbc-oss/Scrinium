# AI, graphics card and OCR

Scrinium brings its own AI: llama.cpp is built in, and the app downloads the model when you click a button. Without a model it keeps working with text recognition, keywords, special rules and heuristics (date, document type, sender); only the folder choice gets weaker. These heuristics are built for German-language documents.

## Model and graphics card

Under *Settings › AI › Model and graphics card*, Scrinium shows each graphics card with its interface (Vulkan) and video memory and recommends a model:

| Detected | Model | Download |
|---|---|---|
| Graphics card with about 10 GB of video memory or more | Qwen3-VL 8B | 5.4 GB |
| Graphics card with about 6 GB of video memory or more | Qwen3-VL 4B | 2.8 GB |
| Integrated graphics with at least 8 GB of shared memory | Qwen3-VL 4B | 2.8 GB |
| Smaller graphics card or processor only | Qwen3-VL 2B | 1.5 GB |

- *Download* fetches the model (Q4_K_M) and the image projector (mmproj, Q8_0) from Hugging Face (`Qwen/Qwen3-VL-*-Instruct-GGUF`). Interrupted downloads are resumed. *Delete* removes them again.
- *Automatic* takes the recommended model if it is downloaded, otherwise the largest smaller one already downloaded. The model stays in video memory between documents.
- The AI runs on Vulkan – on AMD **without ROCm**, and the same way on NVIDIA and Intel. The regular graphics driver is all it needs.
- With a dedicated and an integrated graphics card, only the dedicated one is used.
- If the recommended model does not fit completely into video memory, the recommendation says so and part of it runs on the processor. If loading onto the graphics card fails, Scrinium runs the model on the processor and tells you. Under *Advanced* you can set the *GPU layers* (`-1` = all, `0` = processor only) or a *Custom GGUF file* with its *Image projector*.

Qwen3-VL reads the page image itself, even when it is skewed or blurred, and understands German well. A grammar binds the answer to a JSON schema with fixed choice lists. Scrinium always checks the AI's date against the dates in the text, so it never takes a made-up year.

On a mid-range AMD graphics card, Qwen3-VL 8B takes about 30 seconds per scanned document.

## External AI server

If you prefer LM Studio or Ollama, choose *External server (LM Studio, Ollama …)* under *Settings › AI › Where the AI runs* (`backend = "server"`, OpenAI-compatible, e.g. `http://localhost:1234/v1` for LM Studio, `http://localhost:11434/v1` for Ollama). Load the Instruct model of Qwen3-VL there and keep *Also read the page image* on. Small text-only models work, but choose wrong folders more often.

## OCR

Scrinium reads image-only scans with OCR; it reads PDFs with a text layer and Office files directly. For PDFs it decides page by page: pages without a usable text layer go to OCR, even if other pages have text. Usable means at least 60 characters, most of them readable. If a text layer contains mostly symbols or replacement characters (broken font mapping), Scrinium does not use it and names the page in the notes. Before OCR, it turns every page upright (0°, 90°, 180°, 270°) and straightens skew up to ±15°.

Text recognition uses **Tesseract 5**:

- **Windows:** Tesseract is built into `scrinium.exe` and is unpacked into the cache on first use.
- **Linux:** the system library `libtesseract5`; the `.deb` brings it along with German and English language data. If it is missing with the AppImage or the `.tar.gz`, Scrinium shows the command to install it.

Choose the *Languages* under *Settings › AI › Text recognition (OCR)*: Deutsch, English, Français, Español, Italiano as chips, more under *More* by their Tesseract code (e.g. `nld+pol`). What counts is the language of your documents, not of the interface; a new configuration starts with the system language plus English. Scrinium downloads each chosen language once – with *Accuracy* set to *Fast* from `tessdata_fast` (1–4 MB), with *Best* from `tessdata_best` (4–15 MB, about three times slower) – and checks these five languages with SHA-256. On Linux with *Fast*, Scrinium uses the language data of the `tesseract-ocr-…` packages if all chosen languages are installed. The files are in the cache under `tessdata/`; if your user name contains characters that Tesseract cannot read, they are under `%ProgramData%\Scrinium\tessdata`.

## PDFs

Scrinium opens PDFs with PDFium. It downloads the matching library with the first PDF from [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) (about 4 MB). Without internet access, put `pdfium.dll` (Linux: `libpdfium.so`) next to the program file or into `./lib`, or set `SCRINIUM_PDFIUM=/path`. Image files work without PDFium.

## Network

Downloads check certificates against the system's certificate store. So they also work behind company proxies or antivirus software that inspects HTTPS with its own root certificate.
