//! On Windows, embeds the program icon (`assets/icon.ico`) and the version info in the exe, plus
//! Tesseract (`assets/tesseract/tesseract55.dll`). `main.rs` sets the window icon.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo::rustc-check-cfg=cfg(tesseract_embedded)");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_icon();
        embed_tesseract();
    }
}

/// The Tesseract DLL from the release `tesseract-…` (see `packaging/tesseract`), placed in `assets/tesseract/` by
/// the CI or by `scripts/fetch-tesseract.ps1`; `SCRINIUM_TESSERACT_DLL` points to a different one.
/// Without it, the build yields a Scrinium without OCR – unless `SCRINIUM_REQUIRE_TESSERACT` is set (release).
fn embed_tesseract() {
    println!("cargo:rerun-if-env-changed=SCRINIUM_TESSERACT_DLL");
    println!("cargo:rerun-if-env-changed=SCRINIUM_REQUIRE_TESSERACT");
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dll = std::env::var_os("SCRINIUM_TESSERACT_DLL")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("assets").join("tesseract").join("tesseract55.dll"));
    println!("cargo:rerun-if-changed={}", dll.display());
    let release_file = root.join("packaging").join("tesseract").join("RELEASE");
    println!("cargo:rerun-if-changed={}", release_file.display());
    if dll.is_file() {
        let release = std::fs::read_to_string(&release_file).expect("packaging/tesseract/RELEASE");
        println!("cargo:rustc-cfg=tesseract_embedded");
        println!("cargo:rustc-env=SCRINIUM_TESSERACT_DLL={}", dll.display());
        println!("cargo:rustc-env=SCRINIUM_TESSERACT_RELEASE={}", release.trim());
    } else if std::env::var_os("SCRINIUM_REQUIRE_TESSERACT").is_some() {
        panic!("{} is missing – run scripts/fetch-tesseract.ps1 first", dll.display());
    } else {
        println!("cargo:warning=Built without Tesseract: {} is missing", dll.display());
    }
}

/// winresource takes the version from Cargo.toml. Task Manager shows the description as the name,
/// hence "Scrinium" instead of the package description.
#[cfg(windows)]
fn embed_icon() {
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let icon = root.join("assets").join("icon.ico");
    let mut res = winresource::WindowsResource::new();
    res.set_icon(&icon.to_string_lossy());
    res.set("FileDescription", "Scrinium");
    res.set("ProductName", "Scrinium");
    if let Err(e) = res.compile() {
        panic!("could not embed {}: {e}", icon.display());
    }
}

/// When cross-compiling for Windows from another system, rc.exe is missing; the exe then keeps the
/// default icon.
#[cfg(not(windows))]
fn embed_icon() {}
