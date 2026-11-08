//! Tesseract via its C API, loaded at runtime like PDFium.
//!
//! On Windows the DLL is embedded in scrinium.exe (built with `packaging/tesseract`, one file with all
//! dependencies) and extracted to the cache on first use. On Linux `libtesseract.so.5` comes from the system
//! (package libtesseract5). `SCRINIUM_TESSERACT` points to a different library, e.g. for testing.
//!
//! An `Engine` always belongs to one thread: a Tesseract instance is not thread-safe, so `Engine` is `Send` but not
//! `Sync`.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Result, anyhow, bail};
use image::GrayImage;
use libloading::Library;

use crate::i18n::analysis as texts;

/// Release the embedded DLL comes from (`packaging/tesseract/RELEASE`); also the folder in the cache.
#[cfg(all(windows, tesseract_embedded))]
const RELEASE: &str = env!("SCRINIUM_TESSERACT_RELEASE");

/// Page segmentation like `tesseract --psm 3`: whole page, columns and blocks automatically. The C API otherwise
/// uses 6 (a single block).
const PSM_AUTO: c_int = 3;
/// A single uniform block of text, without column analysis (`--psm 6`).
const PSM_SINGLE_BLOCK: c_int = 6;

type Handle = *mut c_void;

/// The functions of the C API in use (`tesseract/capi.h`).
struct Api {
    version: unsafe extern "C" fn() -> *const c_char,
    create: unsafe extern "C" fn() -> Handle,
    delete: unsafe extern "C" fn(Handle),
    end: unsafe extern "C" fn(Handle),
    init3: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    set_variable: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    set_page_seg_mode: unsafe extern "C" fn(Handle, c_int),
    set_image: unsafe extern "C" fn(Handle, *const u8, c_int, c_int, c_int, c_int),
    set_source_resolution: unsafe extern "C" fn(Handle, c_int),
    utf8_text: unsafe extern "C" fn(Handle) -> *mut c_char,
    tsv_text: unsafe extern "C" fn(Handle, c_int) -> *mut c_char,
    delete_text: unsafe extern "C" fn(*mut c_char),
    loaded_languages: unsafe extern "C" fn(Handle) -> *mut *mut c_char,
    delete_text_array: unsafe extern "C" fn(*mut *mut c_char),
    /// Keeps the library loaded as long as the function pointers live (forever).
    _library: Library,
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

/// Loads Tesseract once for the whole program; analysis and scan processing share the library.
fn api() -> Result<&'static Api> {
    API.get_or_init(|| load().map_err(|e| format!("{e:#}"))).as_ref().map_err(|e| anyhow!("{e}"))
}

fn load() -> Result<Api> {
    let t = texts::t();
    let library = match std::env::var_os("SCRINIUM_TESSERACT").filter(|p| !p.is_empty()) {
        Some(path) => unsafe { Library::new(&path) }.map_err(|e| anyhow!((t.tesseract_load_failed)(&e.to_string())))?,
        None => system_library()?,
    };
    // If a function is missing, it is not the expected library: better report it now than crash later.
    macro_rules! symbol {
        ($name:literal) => {
            *unsafe { library.get(concat!($name, "\0").as_bytes()) }
                .map_err(|e| anyhow!((t.tesseract_load_failed)(&format!("{}: {e}", $name))))?
        };
    }
    Ok(Api {
        version: symbol!("TessVersion"),
        create: symbol!("TessBaseAPICreate"),
        delete: symbol!("TessBaseAPIDelete"),
        end: symbol!("TessBaseAPIEnd"),
        init3: symbol!("TessBaseAPIInit3"),
        set_variable: symbol!("TessBaseAPISetVariable"),
        set_page_seg_mode: symbol!("TessBaseAPISetPageSegMode"),
        set_image: symbol!("TessBaseAPISetImage"),
        set_source_resolution: symbol!("TessBaseAPISetSourceResolution"),
        utf8_text: symbol!("TessBaseAPIGetUTF8Text"),
        tsv_text: symbol!("TessBaseAPIGetTsvText"),
        delete_text: symbol!("TessDeleteText"),
        loaded_languages: symbol!("TessBaseAPIGetLoadedLanguagesAsVector"),
        delete_text_array: symbol!("TessDeleteTextArray"),
        _library: library,
    })
}

/// Windows: the DLL embedded in the exe, extracted to the cache on first use.
#[cfg(windows)]
fn system_library() -> Result<Library> {
    let path = embedded::extract()?;
    unsafe { Library::new(&path) }.map_err(|e| anyhow!((texts::t().tesseract_load_failed)(&e.to_string())))
}

#[cfg(target_os = "linux")]
fn system_library() -> Result<Library> {
    unsafe { Library::new("libtesseract.so.5") }.map_err(|_| anyhow!(texts::t().tesseract_missing_linux))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn system_library() -> Result<Library> {
    bail!(texts::t().tesseract_unsupported)
}

#[cfg(windows)]
mod embedded {
    use std::path::PathBuf;

    use anyhow::Result;

    /// The DLL itself; `build.rs` sets `tesseract_embedded` if it was present at build time.
    #[cfg(tesseract_embedded)]
    static DLL: &[u8] = include_bytes!(env!("SCRINIUM_TESSERACT_DLL"));

    /// Extracts the DLL to `<Cache>/tesseract/<Release>/` if it is missing there or has a different size. Goes
    /// through a temporary file so that a second Scrinium never loads half a DLL.
    #[cfg(tesseract_embedded)]
    pub fn extract() -> Result<PathBuf> {
        let dir = crate::config::cache_dir().join("tesseract").join(super::RELEASE);
        let path = dir.join("tesseract55.dll");
        if std::fs::metadata(&path).is_ok_and(|m| m.len() == DLL.len() as u64) {
            return Ok(path);
        }
        std::fs::create_dir_all(&dir)?;
        let tmp = dir.join(format!("tesseract55.dll.{}.tmp", std::process::id()));
        std::fs::write(&tmp, DLL)?;
        if let Err(e) = std::fs::rename(&tmp, &path) {
            std::fs::remove_file(&tmp).ok();
            // Another Scrinium has just extracted and loaded it: then it is there.
            if !std::fs::metadata(&path).is_ok_and(|m| m.len() == DLL.len() as u64) {
                return Err(e.into());
            }
        }
        Ok(path)
    }

    #[cfg(not(tesseract_embedded))]
    pub fn extract() -> Result<PathBuf> {
        anyhow::bail!(crate::i18n::analysis::t().tesseract_not_embedded)
    }
}

/// Version of the library, e.g. "5.5.3"; loads it to find out.
pub fn version() -> Result<String> {
    let api = api()?;
    Ok(unsafe { CStr::from_ptr((api.version)()) }.to_string_lossy().into_owned())
}

/// A Tesseract instance with a fixed set of loaded languages.
pub struct Engine {
    api: &'static Api,
    handle: Handle,
}

// The instance may move between threads (analysis, scan processing) but is never used concurrently.
unsafe impl Send for Engine {}

impl Engine {
    /// Starts Tesseract with the languages `languages` ("deu+eng") from the folder `tessdata`. If one of the
    /// languages is missing afterwards, its file is unusable: `Err` names it in `missing`.
    pub fn new(tessdata: &Path, languages: &str) -> std::result::Result<Self, InitError> {
        let t = texts::t();
        let api = api().map_err(InitError::Other)?;
        let path = c_path(tessdata).map_err(InitError::Other)?;
        let langs = CString::new(languages).map_err(|e| InitError::Other(e.into()))?;
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err(InitError::Other(anyhow!((t.tesseract_init_failed)(languages))));
        }
        let engine = Engine { api, handle };
        // Keep notes like "Estimating resolution as …" out of the console; Scrinium detects errors itself.
        let quiet = if cfg!(windows) { c"NUL" } else { c"/dev/null" };
        unsafe { (api.set_variable)(handle, c"debug_file".as_ptr(), quiet.as_ptr()) };
        let rc = unsafe { (api.init3)(handle, path.as_ptr(), langs.as_ptr()) };
        // Init3 reports success even if only the first language could be loaded.
        let loaded = engine.loaded_languages();
        let missing: Vec<String> = languages
            .split('+')
            .filter(|l| !l.is_empty() && !loaded.iter().any(|x| x == l))
            .map(String::from)
            .collect();
        if rc != 0 || !missing.is_empty() {
            return Err(InitError::Missing(if missing.is_empty() { vec![languages.to_string()] } else { missing }));
        }
        unsafe { (api.set_page_seg_mode)(handle, PSM_AUTO) };
        Ok(engine)
    }

    fn loaded_languages(&self) -> Vec<String> {
        let arr = unsafe { (self.api.loaded_languages)(self.handle) };
        if arr.is_null() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for i in 0.. {
            let item = unsafe { *arr.add(i) };
            if item.is_null() {
                break;
            }
            out.push(unsafe { CStr::from_ptr(item) }.to_string_lossy().into_owned());
        }
        unsafe { (self.api.delete_text_array)(arr) };
        out
    }

    /// Recognises the text of a greyscale image. `dpi` helps Tesseract estimate the font size.
    pub fn text(&self, img: &GrayImage, dpi: u32) -> Result<String> {
        self.set_image(img, dpi)?;
        self.take(unsafe { (self.api.utf8_text)(self.handle) })
    }

    /// Like `text`, but as TSV with the position of each word (columns as in `tesseract … tsv`, no header row).
    pub fn tsv(&self, img: &GrayImage, dpi: u32) -> Result<String> {
        self.set_image(img, dpi)?;
        self.take(unsafe { (self.api.tsv_text)(self.handle, 0) })
    }

    /// Like `tsv`, but as a single block without column analysis – for comparing the orientations of a page. The
    /// column analysis rotates vertical text blocks by 90° itself and so reads a sideways page even unrotated; a
    /// single block reads only horizontal lines.
    pub fn tsv_single_block(&self, img: &GrayImage, dpi: u32) -> Result<String> {
        unsafe { (self.api.set_page_seg_mode)(self.handle, PSM_SINGLE_BLOCK) };
        let tsv = self.tsv(img, dpi);
        unsafe { (self.api.set_page_seg_mode)(self.handle, PSM_AUTO) };
        tsv
    }

    fn set_image(&self, img: &GrayImage, dpi: u32) -> Result<()> {
        let (w, h) = img.dimensions();
        let (w, h) = (c_int::try_from(w)?, c_int::try_from(h)?);
        unsafe {
            (self.api.set_image)(self.handle, img.as_raw().as_ptr(), w, h, 1, w);
            (self.api.set_source_resolution)(self.handle, dpi.clamp(70, 1200) as c_int);
        }
        Ok(())
    }

    fn take(&self, text: *mut c_char) -> Result<String> {
        if text.is_null() {
            bail!(texts::t().tesseract_no_text);
        }
        let out = unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
        unsafe { (self.api.delete_text)(text) };
        Ok(out)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe {
            (self.api.end)(self.handle);
            (self.api.delete)(self.handle);
        }
    }
}

/// Why Tesseract does not start: missing or broken language data (then download again) or something else.
pub enum InitError {
    Missing(Vec<String>),
    Other(anyhow::Error),
}

/// The path as Tesseract expects it. On Windows, Tesseract opens files in the ANSI code page: a path with umlauts
/// (e.g. in the user name) must be passed as a short 8.3 name or in that code page.
#[cfg(windows)]
pub fn c_path(path: &Path) -> Result<CString> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::Globalization::{CP_ACP, WC_NO_BEST_FIT_CHARS, WideCharToMultiByte};
    use windows::Win32::Storage::FileSystem::GetShortPathNameW;
    use windows::core::{PCSTR, PCWSTR};

    let unusable = || anyhow!((texts::t().tessdata_path_unusable)(&path.display().to_string()));
    if let Some(s) = path.to_str().filter(|s| s.is_ascii()) {
        return CString::new(s).map_err(|_| unusable());
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    let with_nul: Vec<u16> = wide.iter().copied().chain(Some(0)).collect();
    let mut short = vec![0u16; 1024];
    let n = unsafe { GetShortPathNameW(PCWSTR(with_nul.as_ptr()), Some(&mut short)) } as usize;
    if n > 0 && n < short.len() {
        let s = String::from_utf16_lossy(&short[..n]);
        if s.is_ascii() {
            return CString::new(s).map_err(|_| unusable());
        }
    }
    let mut used_default = BOOL(0);
    let mut buf = vec![0u8; wide.len() * 2 + 1];
    let n = unsafe {
        WideCharToMultiByte(CP_ACP, WC_NO_BEST_FIT_CHARS, &wide, Some(&mut buf), PCSTR::null(), Some(&mut used_default))
    };
    if n <= 0 || used_default.as_bool() {
        return Err(unusable());
    }
    buf.truncate(n as usize);
    CString::new(buf).map_err(|_| unusable())
}

/// On Linux, paths are bytes; Tesseract takes them as they are.
#[cfg(not(windows))]
pub fn c_path(path: &Path) -> Result<CString> {
    use std::os::unix::ffi::OsStrExt;
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| anyhow!((texts::t().tessdata_path_unusable)(&path.display().to_string())))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn paths_with_umlauts_become_ansi() {
        let ascii = c_path(Path::new(r"C:\Users\max\tessdata")).unwrap();
        assert_eq!(ascii.as_bytes(), br"C:\Users\max\tessdata");
        // The path does not exist, so no 8.3 name: the umlauts come in the ANSI code page (Windows-1252).
        let umlaut = c_path(Path::new(r"C:\Users\Müller\tessdata")).unwrap();
        assert_eq!(umlaut.as_bytes(), b"C:\\Users\\M\xfcller\\tessdata");
        assert!(c_path(Path::new(r"C:\Users\王\tessdata")).is_err());
    }
}
