# Contributing

Thank you for helping to improve **Scrinium**.

## Prerequisites

1. [Rust](https://rustup.rs/) – `rust-toolchain.toml` sets the version; rustup fetches it by itself.
2. For llama.cpp with Vulkan:

| System | Packages |
|---|---|
| Windows | Visual Studio Build Tools (C++), [CMake](https://cmake.org), [Vulkan SDK](https://vulkan.lunarg.com/sdk/home) (sets `VULKAN_SDK`), LLVM/Clang for bindgen (`winget install LLVM.LLVM`) |
| Linux (Debian/Ubuntu) | `apt install build-essential cmake libclang-dev libvulkan-dev glslc spirv-headers libxkbcommon-dev libwayland-dev libgl1-mesa-dev` |

## Getting started

```bash
git clone https://github.com/fly2nbc-oss/Scrinium.git
cd Scrinium
cargo run --release
```

The first build compiles llama.cpp as well and takes a few minutes.

**Windows with a deep project path:** the Vulkan build of llama.cpp creates paths of up to about 235 characters under `target\`. If the project sits deep in your user folder, these paths break the 260-character limit of MSBuild. Then build through the script:

```powershell
pwsh scripts/cargo-windows.ps1 build --release
```

If needed, it maps the drive `S:` as a short path to the project folder (`subst`; choose another drive with `SCRINIUM_BUILD_DRIVE`), puts CMake from the Build Tools and LLVM on the `PATH` and runs cargo from there. Always build the same way: CMake remembers the path of the first build and otherwise stops with “CMakeCache.txt directory … is different”.

Cargo features choose the GPU backend; the default is `vulkan` (AMD, NVIDIA, Intel):

```bash
cargo build --release --no-default-features                      # processor only
cargo build --release --no-default-features --features rocm      # AMD ROCm/HIP (Linux)
cargo build --release --no-default-features --features cuda      # NVIDIA CUDA
cargo build --release --no-default-features --features metal     # macOS
```

## Checking your build

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo deny --locked check      # vulnerabilities, licenses, sources (deny.toml)
```

On Windows, put `pwsh scripts/cargo-windows.ps1 …` in front. If the debug build of llama.cpp fails there (Visual Studio 18 with CMake 4), check with `--release` – the CI covers the debug profile.

The tests cover date recognition, heuristics, file names and templates, year folders, rules and keywords, Office and text files, the JSON schema of the AI answer, undo, safe paths, skew and rotation detection, the AI client against a simulated server, and resumed downloads. They need neither a model nor a graphics card.

## Text recognition (Tesseract)

**Windows:** `pwsh scripts/fetch-tesseract.ps1` downloads the DLL from the release named in `packaging/tesseract/RELEASE` (needs the GitHub CLI `gh`), checks it against `packaging/tesseract/SHA256SUMS` and puts it into `assets/tesseract/`; `build.rs` embeds it. Without it, Scrinium still builds, only without text recognition (`SCRINIUM_REQUIRE_TESSERACT=1` turns this into an error, as in the CI; `SCRINIUM_TESSERACT_DLL` points to another DLL). The workflow *Tesseract* (started by hand) builds a new DLL with `packaging/tesseract/build.ps1`; then copy the tag and the checksum into `packaging/tesseract/RELEASE` and `SHA256SUMS`.

**Linux:** install `libtesseract5`, `tesseract-ocr-deu` and `tesseract-ocr-eng`.

## Structure

Modules, product decisions and pitfalls are in [`AGENTS.md`](AGENTS.md). The interface follows the shared design system of the desktop apps; `src/theme.rs` holds its values and components, and new views use these instead of the egui defaults. Words and patterns for the language of the documents (dates, document types, senders, legal forms) live in [`src/doclang/`](src/doclang/), one file per document language; Scrinium ships German, and another language is a new file with its own `DocLang`.

## Packages

- **Windows:** `pwsh packaging/windows/build.ps1` after the release build → `dist\scrinium_<version>_x64-setup.exe` and `dist\scrinium_<version>_x64-portable.zip` (NSIS through cargo-packager, settings in `Cargo.toml`).
- **Linux:** `bash packaging/linux/build.sh` after the release build on Ubuntu 24.04 → `.deb` and AppImage in `dist/linux/` (needs cargo-deb, patchelf, curl and python3).

## Continuous Integration

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs on pull requests, pushes to `main`, tags `v*` and by hand (*Actions → CI → Run workflow* or `gh workflow run CI`). A push that changes only `*.md` or `screenshots/` starts no run.

- **Every run:** on Linux `cargo fmt --check`, Clippy without warnings, the tests and `cargo deny`; on Windows Clippy and the tests in the release profile; actionlint and zizmor check the workflows.
- **Packages** (every run except pull requests): release build; for Linux `.tar.gz`, `.deb` and AppImage, for Windows portable ZIP and installer. The CI installs the installer silently, starts it and uninstalls it again (`packaging/windows/test-installer.ps1` – do not run it on your own computer); it installs or unpacks `.deb` and AppImage and starts them. Kept for 7 days as artifacts.
- **Actions** are pinned to commits; Dependabot (`.github/dependabot.yml`) suggests updates for actions and dependencies once a month.
- **Release draft:** a tag `vX.Y.Z` that matches the version in `Cargo.toml` creates a draft with the packages, `SHA256SUMS.txt` and the notes from the matching section in [`CHANGELOG.md`](CHANGELOG.md). Publishing is done by hand.

## Pull requests

- One topic per pull request, with **what** and **why** in the description.
- For changes to the interface, attach a screenshot.
- Format, Clippy and tests must pass; new logic gets a test.
- No real documents, names, addresses or account numbers in tests, examples and screenshots – use placeholders such as “Max Mustermann, Musterstadt”.

## Code of conduct

The [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md) applies.

## License

By contributing, you agree that your contribution is licensed under the project's [Apache-2.0 license](./LICENSE).
