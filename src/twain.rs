//! TWAIN on Windows: scanning with the vendor's dialog (e.g. Epson Scan 2, Canon ScanGear) and all its options;
//! all sheets from the feeder come in one pass. Scrinium talks only to the Windows TWAIN manager
//! (`TWAINDSM.dll`), so every scanner with a 64-bit TWAIN driver works.
//!
//! The flow follows TWAIN 1.x with a message loop: Scrinium registers without `DF_APP2`, then the source sends
//! its events via window messages. Old and new drivers both understand this.

use std::ffi::c_void;
use std::ptr::null_mut;

use anyhow::{Context, Result, anyhow, bail};
use image::DynamicImage;
use windows::Win32::Foundation::{GlobalFree, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, MSG, RegisterClassW,
    TranslateMessage, WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use windows::core::{s, w};

use crate::i18n::scan::t as texts;
use crate::scan::ScannedPage;

const DG_CONTROL: u32 = 0x0001;
const DG_IMAGE: u32 = 0x0002;

const DAT_EVENT: u16 = 0x0002;
const DAT_IDENTITY: u16 = 0x0003;
const DAT_PARENT: u16 = 0x0004;
const DAT_PENDINGXFERS: u16 = 0x0005;
const DAT_USERINTERFACE: u16 = 0x0009;
const DAT_IMAGEINFO: u16 = 0x0101;
const DAT_IMAGENATIVEXFER: u16 = 0x0104;

const MSG_GET: u16 = 0x0001;
const MSG_GETFIRST: u16 = 0x0004;
const MSG_GETNEXT: u16 = 0x0005;
const MSG_RESET: u16 = 0x0007;
const MSG_XFERREADY: u16 = 0x0101;
const MSG_CLOSEDSREQ: u16 = 0x0102;
const MSG_CLOSEDSOK: u16 = 0x0103;
const MSG_OPENDSM: u16 = 0x0301;
const MSG_CLOSEDSM: u16 = 0x0302;
const MSG_OPENDS: u16 = 0x0401;
const MSG_CLOSEDS: u16 = 0x0402;
const MSG_DISABLEDS: u16 = 0x0501;
const MSG_ENABLEDS: u16 = 0x0502;
const MSG_PROCESSEVENT: u16 = 0x0601;
const MSG_ENDXFER: u16 = 0x0701;

const TWRC_SUCCESS: u16 = 0;
const TWRC_CHECKSTATUS: u16 = 2;
const TWRC_CANCEL: u16 = 3;
const TWRC_NOTDSEVENT: u16 = 5;
const TWRC_XFERDONE: u16 = 6;

/// `TW_STR32`: 32 characters, null-terminated, padded to 34 bytes.
type Str32 = [u8; 34];

#[repr(C, packed(2))]
#[derive(Clone, Copy)]
struct Version {
    major: u16,
    minor: u16,
    language: u16,
    country: u16,
    info: Str32,
}

/// `TW_IDENTITY`: who is talking (Scrinium) and to whom (the source, i.e. the scanner).
#[repr(C, packed(2))]
#[derive(Clone, Copy)]
struct Identity {
    id: u32,
    version: Version,
    protocol_major: u16,
    protocol_minor: u16,
    supported_groups: u32,
    manufacturer: Str32,
    product_family: Str32,
    product_name: Str32,
}

#[repr(C, packed(2))]
struct UserInterface {
    show_ui: u16,
    modal_ui: u16,
    parent: *mut c_void,
}

#[repr(C, packed(2))]
struct PendingXfers {
    count: u16,
    eoj: u32,
}

#[repr(C, packed(2))]
struct Event {
    event: *mut c_void,
    message: u16,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Default)]
struct Fix32 {
    whole: i16,
    frac: u16,
}

#[repr(C, packed(2))]
#[derive(Default)]
struct ImageInfo {
    x_resolution: Fix32,
    y_resolution: Fix32,
    width: i32,
    length: i32,
    samples_per_pixel: i16,
    bits_per_sample: [i16; 8],
    bits_per_pixel: i16,
    planar: u16,
    pixel_type: i16,
    compression: u16,
}

type DsmEntry = unsafe extern "system" fn(*mut Identity, *mut Identity, u32, u16, u16, *mut c_void) -> u16;

fn str32(s: &str) -> Str32 {
    let mut out = [0u8; 34];
    for (o, b) in out.iter_mut().zip(s.bytes().take(32)) {
        *o = b;
    }
    out
}

fn read_str32(s: &Str32) -> String {
    let end = s.iter().position(|b| *b == 0).unwrap_or(s.len());
    // TWAIN names are ANSI; Latin-1 is enough for the usual vendor names.
    s[..end].iter().map(|&b| b as char).collect()
}

fn empty_identity() -> Identity {
    Identity {
        id: 0,
        version: Version { major: 0, minor: 0, language: 0, country: 0, info: [0; 34] },
        protocol_major: 0,
        protocol_minor: 0,
        supported_groups: 0,
        manufacturer: [0; 34],
        product_family: [0; 34],
        product_name: [0; 34],
    }
}

/// Scrinium as a TWAIN application (protocol 1.9, without `DF_APP2`, see above).
fn app_identity() -> Identity {
    let mut id = empty_identity();
    let (major, minor) = env!("CARGO_PKG_VERSION").split_once('.').unwrap_or(("0", "0"));
    id.version = Version {
        major: major.parse().unwrap_or(0),
        minor: minor.split('.').next().and_then(|m| m.parse().ok()).unwrap_or(0),
        language: 0xFFFF, // TWLG_USERLOCALE
        country: 49,      // TWCY_GERMANY
        info: str32(env!("CARGO_PKG_VERSION")),
    };
    id.protocol_major = 1;
    id.protocol_minor = 9;
    id.supported_groups = DG_CONTROL | DG_IMAGE;
    id.manufacturer = str32("fly2nbc-oss");
    id.product_family = str32("Scrinium");
    id.product_name = str32("Scrinium");
    id
}

/// An invisible window: parent of the source's dialog and target of its messages.
struct Window(HWND);

impl Window {
    fn new() -> Result<Self> {
        unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                lpszClassName: w!("ScriniumTwain"),
                ..Default::default()
            };
            // Fails if the class is already registered; the window is created anyway.
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("ScriniumTwain"),
                w!("Scrinium – Scan"),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                0,
                0,
                None,
                None,
                instance,
                None,
            )?;
            Ok(Self(hwnd))
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

/// The opened TWAIN manager; closes itself when dropped.
struct Manager {
    entry: DsmEntry,
    app: Identity,
    parent: HWND,
    _window: Window,
}

impl Manager {
    /// `None`: there is no TWAIN manager on this computer.
    fn open() -> Result<Option<Self>> {
        let entry = unsafe {
            let Ok(dll) = LoadLibraryW(w!("TWAINDSM.dll")) else { return Ok(None) };
            match GetProcAddress(dll, s!("DSM_Entry")) {
                Some(f) => std::mem::transmute::<unsafe extern "system" fn() -> isize, DsmEntry>(f),
                None => return Ok(None),
            }
        };
        let window = Window::new()?;
        let mut manager = Self { entry, app: app_identity(), parent: window.0, _window: window };
        let mut parent = manager.parent.0;
        let rc = manager.call(None, DG_CONTROL, DAT_PARENT, MSG_OPENDSM, &mut parent as *mut _ as *mut c_void);
        if rc != TWRC_SUCCESS {
            bail!((texts().twain_manager_failed)(rc));
        }
        Ok(Some(manager))
    }

    fn call(&mut self, source: Option<&mut Identity>, dg: u32, dat: u16, msg: u16, data: *mut c_void) -> u16 {
        let dest = source.map_or(null_mut(), |s| s as *mut Identity);
        unsafe { (self.entry)(&mut self.app, dest, dg, dat, msg, data) }
    }

    /// All TWAIN sources (scanners) by name.
    fn sources(&mut self) -> Vec<String> {
        let mut names = Vec::new();
        let mut id = empty_identity();
        let mut rc = self.call(None, DG_CONTROL, DAT_IDENTITY, MSG_GETFIRST, &mut id as *mut _ as *mut c_void);
        while rc == TWRC_SUCCESS {
            let name = read_str32(&id.product_name);
            if !name.is_empty() && !names.contains(&name) {
                names.push(name);
            }
            id = empty_identity();
            rc = self.call(None, DG_CONTROL, DAT_IDENTITY, MSG_GETNEXT, &mut id as *mut _ as *mut c_void);
        }
        names
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        let mut parent = self.parent.0;
        self.call(None, DG_CONTROL, DAT_PARENT, MSG_CLOSEDSM, &mut parent as *mut _ as *mut c_void);
    }
}

/// Names of the installed TWAIN scanners; empty if there is no TWAIN manager.
pub fn sources() -> Result<Vec<String>> {
    Ok(Manager::open()?.map(|mut m| m.sources()).unwrap_or_default())
}

/// Scans with the TWAIN source `name` (empty = the default source) and its own dialog. `Ok(None)`: no TWAIN
/// scanner present, WIA takes over. `Ok(Some(empty))`: cancelled in the dialog.
pub fn acquire(name: &str) -> Result<Option<Vec<ScannedPage>>> {
    let Some(mut manager) = Manager::open()? else { return Ok(None) };
    let available = manager.sources();
    if available.is_empty() {
        return Ok(None);
    }
    // The selected source, otherwise the TWAIN manager's default source.
    let mut source = empty_identity();
    if available.iter().any(|a| a == name) {
        source.product_name = str32(name);
    }
    let rc = manager.call(None, DG_CONTROL, DAT_IDENTITY, MSG_OPENDS, &mut source as *mut _ as *mut c_void);
    if rc != TWRC_SUCCESS {
        bail!((texts().twain_open_failed)(&read_str32(&source.product_name), rc));
    }
    let result = scan_with_dialog(&mut manager, &mut source);
    manager.call(None, DG_CONTROL, DAT_IDENTITY, MSG_CLOSEDS, &mut source as *mut _ as *mut c_void);
    result.map(Some)
}

/// Shows the source's dialog and fetches all pages as soon as its scan button is pressed.
fn scan_with_dialog(manager: &mut Manager, source: &mut Identity) -> Result<Vec<ScannedPage>> {
    let mut ui = UserInterface { show_ui: 1, modal_ui: 0, parent: manager.parent.0 };
    let rc = manager.call(Some(source), DG_CONTROL, DAT_USERINTERFACE, MSG_ENABLEDS, &mut ui as *mut _ as *mut c_void);
    match rc {
        TWRC_SUCCESS | TWRC_CHECKSTATUS => {}
        TWRC_CANCEL => return Ok(Vec::new()),
        _ => bail!((texts().twain_dialog_failed)(rc)),
    }
    let mut pages = Vec::new();
    let mut error = None;
    let mut msg = MSG::default();
    // The message loop drives the dialog; every message goes to the source first.
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        let mut event = Event { event: &mut msg as *mut MSG as *mut c_void, message: 0 };
        let rc =
            manager.call(Some(source), DG_CONTROL, DAT_EVENT, MSG_PROCESSEVENT, &mut event as *mut _ as *mut c_void);
        match event.message {
            MSG_XFERREADY => {
                if let Err(e) = transfer_all(manager, source, &mut pages) {
                    error = Some(e);
                }
                break;
            }
            MSG_CLOSEDSREQ | MSG_CLOSEDSOK => break,
            _ => {}
        }
        if rc == TWRC_NOTDSEVENT {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    let mut ui = UserInterface { show_ui: 0, modal_ui: 0, parent: manager.parent.0 };
    manager.call(Some(source), DG_CONTROL, DAT_USERINTERFACE, MSG_DISABLEDS, &mut ui as *mut _ as *mut c_void);
    match error {
        Some(e) if pages.is_empty() => Err(e),
        _ => Ok(pages),
    }
}

/// Fetches all pending pages (with the feeder, all sheets) as images.
fn transfer_all(manager: &mut Manager, source: &mut Identity, pages: &mut Vec<ScannedPage>) -> Result<()> {
    loop {
        let mut info = ImageInfo::default();
        let has_info = manager.call(Some(source), DG_IMAGE, DAT_IMAGEINFO, MSG_GET, &mut info as *mut _ as *mut c_void)
            == TWRC_SUCCESS;
        let mut handle: *mut c_void = null_mut();
        let rc =
            manager.call(Some(source), DG_IMAGE, DAT_IMAGENATIVEXFER, MSG_GET, &mut handle as *mut _ as *mut c_void);
        let mut page = None;
        match rc {
            TWRC_XFERDONE => {
                let dib = unsafe { take_dib(handle) };
                let x_res = info.x_resolution;
                let dpi = if has_info { f32::from(x_res.whole) + f32::from(x_res.frac) / 65536.0 } else { 0.0 };
                page = Some(dib.and_then(|d| dib_to_page(&d, dpi)));
            }
            TWRC_CANCEL => {}
            _ => {
                let mut pending = PendingXfers { count: 0, eoj: 0 };
                manager.call(
                    Some(source),
                    DG_CONTROL,
                    DAT_PENDINGXFERS,
                    MSG_RESET,
                    &mut pending as *mut _ as *mut c_void,
                );
                bail!((texts().twain_transfer_failed)(rc));
            }
        }
        let mut pending = PendingXfers { count: 0, eoj: 0 };
        manager.call(Some(source), DG_CONTROL, DAT_PENDINGXFERS, MSG_ENDXFER, &mut pending as *mut _ as *mut c_void);
        if let Some(page) = page {
            pages.push(page?);
        }
        if pending.count == 0 {
            return Ok(());
        }
    }
}

/// Copies the DIB out of the source's memory block and frees it.
unsafe fn take_dib(handle: *mut c_void) -> Result<Vec<u8>> {
    let mem = HGLOBAL(handle);
    if mem.is_invalid() {
        bail!(texts().twain_empty_image);
    }
    unsafe {
        let size = GlobalSize(mem);
        let ptr = GlobalLock(mem) as *const u8;
        if ptr.is_null() || size == 0 {
            let _ = GlobalFree(mem);
            bail!(texts().twain_image_unreadable);
        }
        let bytes = std::slice::from_raw_parts(ptr, size).to_vec();
        let _ = GlobalUnlock(mem);
        let _ = GlobalFree(mem);
        Ok(bytes)
    }
}

/// A DIB (BITMAPINFOHEADER, colour table, pixels) as a page: with a BMP file header in front, the `image` crate
/// reads it like a BMP file. The resolution comes from the scanner, otherwise from the DIB.
pub fn dib_to_page(dib: &[u8], dpi: f32) -> Result<ScannedPage> {
    let u32_at = |i: usize| dib.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let header = u32_at(0).context(texts().twain_image_short)? as usize;
    let bit_count = dib.get(14..16).map(|b| u16::from_le_bytes([b[0], b[1]])).unwrap_or(24);
    let compression = u32_at(16).unwrap_or(0);
    let x_ppm = u32_at(24).unwrap_or(0) as i32;
    let colors_used = u32_at(32).unwrap_or(0) as usize;
    let palette = match (bit_count, colors_used) {
        (b, 0) if b <= 8 => 1usize << b,
        (_, n) => n,
    } * 4;
    // BI_BITFIELDS with the short header: three colour masks follow the header.
    let masks = if compression == 3 && header == 40 { 12 } else { 0 };
    let offset = 14 + header + masks + palette;
    let mut bmp = Vec::with_capacity(14 + dib.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&((14 + dib.len()) as u32).to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&(offset as u32).to_le_bytes());
    bmp.extend_from_slice(dib);
    let image: DynamicImage = image::load_from_memory_with_format(&bmp, image::ImageFormat::Bmp)
        .map_err(|e| anyhow!((texts().twain_image_invalid)(&e.to_string())))?;
    let dpi = if dpi >= 50.0 {
        dpi
    } else if x_ppm > 0 {
        x_ppm as f32 * 0.0254
    } else {
        0.0
    };
    Ok(ScannedPage { image, dpi: crate::scan::sane_dpi(dpi) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structures_match_twain_h() {
        // Packed to 2 bytes as in twain.h; pointers are 8 bytes.
        assert_eq!(size_of::<Identity>(), 156);
        assert_eq!(size_of::<UserInterface>(), 12);
        assert_eq!(size_of::<PendingXfers>(), 6);
        assert_eq!(size_of::<Event>(), 10);
        assert_eq!(size_of::<ImageInfo>(), 42);
    }

    #[test]
    fn names_fit_into_32_characters() {
        let s = str32("EPSON ET-3850 Series");
        assert_eq!(read_str32(&s), "EPSON ET-3850 Series");
        let long = str32(&"x".repeat(40));
        assert_eq!(read_str32(&long).len(), 32);
    }

    fn dib(width: i32, height: i32, bits: u16, palette: &[[u8; 4]], pixels: &[u8], ppm: i32) -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(&40u32.to_le_bytes());
        d.extend_from_slice(&width.to_le_bytes());
        d.extend_from_slice(&height.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&bits.to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
        d.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
        d.extend_from_slice(&ppm.to_le_bytes());
        d.extend_from_slice(&ppm.to_le_bytes());
        d.extend_from_slice(&(palette.len() as u32).to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes());
        for c in palette {
            d.extend_from_slice(c);
        }
        d.extend_from_slice(pixels);
        d
    }

    #[test]
    fn color_dib_becomes_a_page() {
        // 2 × 2 pixels, 24 bit, rows padded to 4 bytes, bottom to top, BGR.
        let rows = [[0u8, 0, 255, 0, 255, 0, 0, 0], [255, 0, 0, 255, 255, 255, 0, 0]];
        let page = dib_to_page(&dib(2, 2, 24, &[], &rows.concat(), 11811), 0.0).unwrap();
        let rgb = page.image.to_rgb8();
        assert_eq!(rgb.dimensions(), (2, 2));
        assert_eq!(rgb.get_pixel(0, 1).0, [255, 0, 0], "bottom row first");
        assert_eq!(rgb.get_pixel(1, 0).0, [255, 255, 255]);
        assert!((page.dpi - 300.0).abs() < 1.0, "11811 dots per metre are 300 dpi");
    }

    #[test]
    fn gray_dib_with_palette_uses_the_scanner_resolution() {
        let palette: Vec<[u8; 4]> = (0..=255u8).map(|v| [v, v, v, 0]).collect();
        let pixels = [10u8, 200, 0, 0, 30, 40, 0, 0];
        let page = dib_to_page(&dib(2, 2, 8, &palette, &pixels, 0), 200.0).unwrap();
        assert_eq!(page.image.to_luma8().get_pixel(1, 1).0, [200]);
        assert_eq!(page.dpi, 200.0);
    }
}
