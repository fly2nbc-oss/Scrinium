//! Scanning: fetch pages from the scanner. On Windows preferably via TWAIN with the vendor's dialog (all sheets
//! from the feeder in one pass), otherwise via WIA with the Windows scan dialog; on Linux via SANE (`scanimage`)
//! with device, resolution and colour from the settings. `scan_pdf` turns them into a PDF.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};

use anyhow::{Context, Result, bail};
use eframe::egui;
use image::DynamicImage;

use crate::config::ScanConfig;

/// A scanned page with its resolution (dots per inch).
pub struct ScannedPage {
    pub image: DynamicImage,
    pub dpi: f32,
}

/// Resolution when the scanner reports none.
#[cfg(any(windows, test))]
const DEFAULT_DPI: f32 = 300.0;

/// Result of a pass: the pages, empty if the dialog was cancelled.
pub type ScanResult = Result<Vec<ScannedPage>, String>;

/// Scans in the background; otherwise the scanner's dialog blocks the UI.
pub fn start(ctx: &egui::Context, work_dir: PathBuf, settings: ScanConfig) -> Receiver<ScanResult> {
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let result = acquire(&work_dir, &settings).map_err(|e| format!("{e:#}"));
        let _ = tx.send(result);
        ctx.request_repaint();
    });
    rx
}

/// A scanner to choose in the settings: value for `scan.device` and name.
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub id: String,
    pub label: String,
}

static DEVICES: OnceLock<Vec<Device>> = OnceLock::new();
static DEVICES_REQUESTED: AtomicBool = AtomicBool::new(false);

/// The scanners of this computer. The search runs once in the background; until then `None`.
pub fn devices(ctx: &egui::Context) -> Option<&'static [Device]> {
    if let Some(d) = DEVICES.get() {
        return Some(d);
    }
    if !DEVICES_REQUESTED.swap(true, Ordering::SeqCst) {
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = DEVICES.set(find_devices());
            ctx.request_repaint();
        });
    }
    None
}

/// Windows: the TWAIN scanners, plus the Windows dialog if there are WIA scanners.
#[cfg(windows)]
pub fn find_devices() -> Vec<Device> {
    let t = crate::i18n::scan::t();
    let mut list: Vec<Device> = crate::twain::sources()
        .unwrap_or_default()
        .into_iter()
        .map(|name| Device { id: format!("twain:{name}"), label: format!("{name} (TWAIN)") })
        .collect();
    let wia = wia_scanners();
    if !wia.is_empty() {
        list.push(Device { id: "wia".into(), label: (t.wia_device)(&wia.join(", ")) });
    }
    list
}

/// Names of the WIA scanners (device type 1).
#[cfg(windows)]
fn wia_scanners() -> Vec<String> {
    let script = "try { $m = New-Object -ComObject WIA.DeviceManager; foreach ($d in $m.DeviceInfos) { \
                  if ($d.Type -eq 1) { $d.Properties.Item('Name').Value } } } catch {}";
    powershell(script)
        .map(|out| out.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

/// Linux: the SANE devices from `scanimage -L`.
#[cfg(not(windows))]
pub fn find_devices() -> Vec<Device> {
    let Ok(out) = std::process::Command::new("scanimage").args(["-f", "%d\t%v %m%n"]).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(id, name)| Device { id: format!("sane:{id}"), label: format!("{} ({id})", name.trim()) })
        .collect()
}

/// Runs a PowerShell script without a console window and returns its output.
#[cfg(windows)]
fn powershell(script: &str) -> Result<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .context(crate::i18n::scan::t().powershell_failed)?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Windows: TWAIN preferred, WIA as fallback – and when the Windows dialog is selected.
#[cfg(windows)]
fn acquire(work_dir: &Path, settings: &ScanConfig) -> Result<Vec<ScannedPage>> {
    let device = settings.device.trim();
    if device != "wia" {
        let name = device.strip_prefix("twain:").unwrap_or_default();
        if let Some(pages) = crate::twain::acquire(name)? {
            return Ok(pages);
        }
    }
    Ok(acquire_wia(work_dir)?.into_iter().collect())
}

/// WIA via PowerShell: choose the scanner (only if there are several), Windows scan dialog, result as BMP, which
/// every WIA driver delivers. The last line tells how it ended.
#[cfg(windows)]
const WIA_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
try {
  $dialog = New-Object -ComObject WIA.CommonDialog
  $image = $dialog.ShowAcquireImage(1, 0, 0, '{B96B3CAB-0728-11D3-9D7B-0000F81EF32E}', $false, $true, $false)
  if ($null -eq $image) { 'CANCELLED'; exit 0 }
  $image.SaveFile('{PATH}')
  'OK ' + $image.HorizontalResolution.ToString([cultureinfo]::InvariantCulture)
} catch {
  $e = $_.Exception
  while ($e.InnerException) { $e = $e.InnerException }
  'ERROR ' + $e.HResult.ToString('X8') + ' ' + $e.Message
  exit 1
}
"#;

#[cfg(windows)]
fn acquire_wia(work_dir: &Path) -> Result<Option<ScannedPage>> {
    std::fs::create_dir_all(work_dir)?;
    let file = work_dir.join(format!("scan-{}.bmp", std::process::id()));
    let _ = std::fs::remove_file(&file);
    let stdout = powershell(&WIA_SCRIPT.replace("{PATH}", &file.display().to_string().replace('\'', "''")))?;
    let last = stdout.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or_default();
    if last == "CANCELLED" {
        return Ok(None);
    }
    if let Some(dpi) = last.strip_prefix("OK ") {
        let image = image::open(&file).context(crate::i18n::scan::t().scan_unreadable)?;
        let _ = std::fs::remove_file(&file);
        return Ok(Some(ScannedPage { image, dpi: parse_dpi(dpi) }));
    }
    bail!("{}", wia_message(last.strip_prefix("ERROR ").unwrap_or(last)))
}

/// WIA errors in words; the most common ones by their code.
#[cfg(windows)]
fn wia_message(raw: &str) -> String {
    let t = crate::i18n::scan::t();
    let code = raw.split_whitespace().next().unwrap_or_default();
    match code {
        "80210015" => t.wia_no_scanner.into(),
        "80210005" => t.wia_unreachable.into(),
        "80210006" => t.wia_busy.into(),
        "80210003" => t.wia_no_paper.into(),
        "80210002" => t.wia_paper_jam.into(),
        "80210001" => t.wia_device_error.into(),
        "80040154" => t.wia_missing.into(),
        "" => t.wia_no_answer.into(),
        _ => raw.to_string(),
    }
}

/// SANE: the selected device (otherwise the first) with resolution and colour from the settings, without dialog.
#[cfg(not(windows))]
fn acquire(_work_dir: &Path, settings: &ScanConfig) -> Result<Vec<ScannedPage>> {
    let t = crate::i18n::scan::t();
    let dpi = settings.resolution.clamp(75, 1200);
    let run = |with_mode: bool| {
        let mut cmd = std::process::Command::new("scanimage");
        cmd.args(["--format=png", &format!("--resolution={dpi}")]);
        if let Some(device) = settings.device.strip_prefix("sane:") {
            cmd.args(["-d", device]);
        }
        if with_mode {
            cmd.arg(if settings.color { "--mode=Color" } else { "--mode=Gray" });
        }
        cmd.output()
    };
    let out = match run(true) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => bail!(t.sane_missing),
        r => r.context(t.scanimage_failed)?,
    };
    // Some drivers name the colour modes differently; then use their default.
    let out = if out.status.success() { out } else { run(false).context(t.scanimage_failed)? };
    if !out.status.success() {
        bail!((t.scan_failed)(String::from_utf8_lossy(&out.stderr).trim()));
    }
    let image = image::load_from_memory(&out.stdout).context(t.scan_unreadable)?;
    Ok(vec![ScannedPage { image, dpi: dpi as f32 }])
}

/// Resolution in dpi; nonsense becomes the default resolution. Only TWAIN and WIA report one; SANE scans with the
/// configured one.
#[cfg(any(windows, test))]
pub fn sane_dpi(dpi: f32) -> f32 {
    if (50.0..=2400.0).contains(&dpi) { dpi } else { DEFAULT_DPI }
}

#[cfg(any(windows, test))]
fn parse_dpi(s: &str) -> f32 {
    sane_dpi(s.trim().replace(',', ".").parse::<f32>().unwrap_or(0.0))
}

/// File name for a new scan, e.g. `Scan_2026-10-12_134512.pdf`.
pub fn file_name(now: chrono::NaiveDateTime) -> String {
    format!("Scan_{}.pdf", now.format("%Y-%m-%d_%H%M%S"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_falls_back_to_300() {
        assert_eq!(parse_dpi("300"), 300.0);
        assert_eq!(parse_dpi(" 200,0 "), 200.0);
        assert_eq!(parse_dpi("0"), DEFAULT_DPI);
        assert_eq!(parse_dpi("x"), DEFAULT_DPI);
    }

    #[test]
    fn scan_files_are_named_by_time() {
        let t = chrono::NaiveDate::from_ymd_opt(2026, 10, 12).unwrap().and_hms_opt(13, 45, 12).unwrap();
        assert_eq!(file_name(t), "Scan_2026-10-12_134512.pdf");
    }
}
