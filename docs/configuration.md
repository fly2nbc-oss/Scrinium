# Configuration and command line

## The configuration file

Everything on the settings tabs except *General* is stored in one TOML file. Scrinium creates it on first start; *Configuration file* at the bottom left of the settings shows the path and can open it in an editor:

| System | Path |
|---|---|
| Windows | `%APPDATA%\scrinium\Scrinium\config\config.toml` |
| Linux | `~/.config/scrinium/config.toml` |

Next to it are `view.toml` (`appearance` `light`, `dark` or `system`; `language` `de`, `en`, `fr`, `es` or `it`, without an entry the system language; `list` shown or hidden) and `updates.toml` (update check: allowed or not, last check), plus `decisions.jsonl` (see [Learning from decisions](archive.md#learning-from-decisions)). Models, the language data for text recognition, PDFium and, on Windows, Tesseract are in the cache folder (`%LOCALAPPDATA%\scrinium\Scrinium\cache`, Linux `~/.cache/scrinium`).

The complete template with comments is [`config.example.toml`](../config.example.toml). An excerpt:

```toml
archive_root = "D:/Ablage"
filename_template = "{date}_{doctype}_{sender}_{subject}_{person}_{extra}"
date_format = "%y%m%d"
compact_filenames = true
max_filename_length = 80
fallback_folder = "00_Eingang"                    # when nothing fits
allow_new_folders = false                         # the AI must choose existing folders
exclude = ["_Archiv_alt"]                         # folders not used, with their subfolders

[[persons]]
name = "Max"
aliases = ["Maximilian", "MM"]

[folder_notes]
"Versicherungen" = "Alle Schreiben von Versicherungen: Policen, Beitragsrechnungen, Kündigungen"

[folder_words]
"Versicherungen" = "Versicherungsschein, Police, -Kontoauszug"

[llm]
backend = "embedded"                              # or "server" for LM Studio/Ollama
model_id = ""                                     # empty = to match the GPU, otherwise e.g. "qwen3-vl-4b"
gpu_layers = -1                                   # -1 = everything on the GPU, 0 = processor only
context_size = 12288
use_vision = true

[scan]
device = ""                                       # empty = automatic; or "twain:<name>", "wia", "sane:<device>"
folder = ""                                       # empty = Downloads folder; scans are moved when filed
resolution = 300                                  # SANE only (Linux)
color = true                                      # SANE only (Linux)
preset = "color"                                  # or "bw", "brochure", "photo"
text_layer = true                                 # searchable PDF

[[rules]]
name = "Strom"
keywords = ["Stadtwerke", "Stromabrechnung"]
exclude_keywords = ["Kontoauszug"]
folder = "Wohnung/Nebenkosten/{year}"
```

Saving in the app rewrites the file. The comments are lost; if the old file had comments, it stays next to it as `config.toml.<timestamp>.bak`.

## Environment variables

| Variable | Effect |
|---|---|
| `SCRINIUM_CONFIG=/path/config.toml` | use another configuration file – handy for trying rules on a copy of the archive |
| `SCRINIUM_SETTINGS=archive` | opens the settings on this tab at start (`archive`, `names`, `rules`, `ai`, `scan`, `setup`, `general`), e.g. for screenshots |
| `SCRINIUM_PDFIUM=/path` | load the PDFium library from there |
| `SCRINIUM_TESSERACT=/path/libtesseract.so.5` | load this Tesseract library instead of the built-in one (Windows) or the system one (Linux) |
| `SCRINIUM_TESSDATA=/path` | store and look for Tesseract language data there (in `fast/` or `best/`) instead of the cache |
| `SCRINIUM_LLAMA_LOG=1` | show the log output of llama.cpp (hidden by default; any value turns it on) |

Files or folders given as arguments (`scrinium letter.pdf invoice.jpg`) go into the queue at start.

## Command line

```bash
scrinium --analyze scan.pdf more.jpg     # one JSON line per document; nothing is moved
scrinium --analyze --no-ai *.pdf         # rules and heuristics only, fast
scrinium --setup archive.md              # settings from a description of the archive (TOML on stdout, nothing is saved)
scrinium --setup --json archive.md       # the raw answer of the AI
scrinium --report                        # how often suggestions were kept or changed (decisions.jsonl or a given file)
scrinium --gpu                           # devices, video memory, recommendation, installed models
scrinium --scanner                       # scanners found (TWAIN, WIA, SANE) with their value for scan.device
scrinium --pdf letter.pdf p1.jpg p2.jpg --mode bw  # process images like a scan into a searchable PDF
scrinium --download-model auto           # download the recommended model (or qwen3-vl-2b / -4b / -8b)
scrinium --help                          # all options and the path of the configuration file
```

The command line uses the same configuration file as the window (or `SCRINIUM_CONFIG`). Messages and progress go to stderr, results to stdout.

`--analyze` prints one JSON object per document. It is useful to check rules against a whole archive and to compare models. The keys:

| Key | Content |
|---|---|
| `file`, `kind` | input file and its kind (PDF, image, Office …) |
| `date`, `doctype`, `sender`, `subject`, `person`, `extra` | the suggested details |
| `summary` | the AI's summary of the document |
| `folder`, `folder_notes`, `folder_why`, `folder_exists` | target folder, its description, why it was chosen, and whether it exists |
| `filename` | the suggested file name, without extension |
| `source`, `rule`, `confidence` | where the suggestion came from, the matching special rule, the AI's confidence |
| `ocr`, `corrections`, `warnings` | where the text came from (PDF text layer, Tesseract and its languages), image corrections, notes |
| `seconds`, `ai_time` | total time and AI timings |
| `text` | the first 4,000 characters of the recognised text |
