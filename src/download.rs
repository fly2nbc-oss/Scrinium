//! Downloads of large files with resume (HTTP Range), progress and cancellation.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};

use crate::i18n::ai;

#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub label: String,
    pub done: u64,
    pub total: Option<u64>,
}

impl Progress {
    pub fn fraction(&self) -> Option<f32> {
        self.total.filter(|t| *t > 0).map(|t| (self.done as f64 / t as f64).clamp(0.0, 1.0) as f32)
    }

    pub fn describe(&self) -> String {
        let t = ai::t();
        match self.total {
            Some(total) => (t.progress_of)(&self.label, &format_bytes(self.done), &format_bytes(total)),
            None => (t.progress_done)(&self.label, &format_bytes(self.done)),
        }
    }
}

/// Size with the language's decimal separator and unit, e.g. "1,45 GB".
pub fn format_bytes(b: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let t = ai::t();
    let b = b as f64;
    let (number, unit) =
        if b >= GB { (format!("{:.2}", b / GB), t.unit_gb) } else { (format!("{:.0}", b / MB), t.unit_mb) };
    format!("{} {unit}", number.replace('.', t.decimal_separator))
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
        .build()
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".part");
    dest.with_file_name(name)
}

/// Downloads `url` to `dest`. An interrupted download (`<dest>.part`) is resumed.
/// The file is renamed only once everything has arrived, so `dest` is never half finished.
pub fn download_file(
    url: &str,
    dest: &Path,
    label: &str,
    progress: &mut dyn FnMut(&Progress),
    cancel: &AtomicBool,
) -> Result<()> {
    let t = ai::t();
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).with_context(|| (t.cannot_create)(&dir.display().to_string()))?;
    }
    let part = part_path(dest);
    let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);

    let mut req = agent().get(url);
    if have > 0 {
        req = req.set("Range", &format!("bytes={have}-"));
    }
    let resp = match req.call() {
        Ok(r) => r,
        // Range beyond the end of the file: the partial download is already complete.
        Err(ureq::Error::Status(416, _)) if have > 0 => {
            std::fs::rename(&part, dest)?;
            return Ok(());
        }
        Err(ureq::Error::Status(code, _)) => bail!((t.download_http_failed)(url, code)),
        Err(e) => bail!((t.download_failed)(url, &e.to_string())),
    };

    let resumed = resp.status() == 206;
    let start = if resumed { have } else { 0 };
    let total = if resumed {
        resp.header("Content-Range").and_then(|r| r.rsplit('/').next()).and_then(|t| t.trim().parse().ok())
    } else {
        resp.header("Content-Length").and_then(|l| l.trim().parse::<u64>().ok())
    };

    let mut file: File = if resumed {
        OpenOptions::new().append(true).open(&part)?
    } else {
        File::create(&part).with_context(|| (t.cannot_write)(&part.display().to_string()))?
    };

    let mut state = Progress { label: label.to_string(), done: start, total };
    progress(&state);
    let mut reader = resp.into_reader();
    let mut buf = vec![0u8; 256 * 1024];
    let mut since_report = 0usize;
    loop {
        if cancel.load(Ordering::Relaxed) {
            file.flush()?;
            bail!(t.download_cancelled);
        }
        let n = reader.read(&mut buf).map_err(|e| anyhow!((t.download_interrupted)(&e.to_string())))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        state.done += n as u64;
        since_report += n;
        if since_report >= 1024 * 1024 {
            since_report = 0;
            progress(&state);
        }
    }
    file.flush()?;
    drop(file);
    progress(&state);

    if let Some(total) = total
        && state.done < total
    {
        bail!((t.download_incomplete)(&format_bytes(state.done), &format_bytes(total)));
    }
    std::fs::rename(&part, dest).with_context(|| (t.cannot_rename)(&part.display().to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    /// Serves `data`, honours `Range: bytes=N-` and cuts off the first response after `cut` bytes.
    fn range_server(data: Vec<u8>, cut: usize) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for round in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut from = 0usize;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_lowercase().strip_prefix("range: bytes=") {
                        from = v.trim().trim_end_matches('-').parse().unwrap();
                    }
                }
                let body = &data[from..];
                let head = if from > 0 {
                    format!(
                        "HTTP/1.1 206 Partial\r\nContent-Length: {}\r\nContent-Range: bytes {from}-{}/{}\r\nConnection: close\r\n\r\n",
                        body.len(),
                        data.len() - 1,
                        data.len()
                    )
                } else {
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())
                };
                stream.write_all(head.as_bytes()).unwrap();
                let send = if round == 0 { &body[..cut] } else { body };
                stream.write_all(send).unwrap();
            }
        });
        format!("http://{addr}/datei.bin")
    }

    #[test]
    fn resumes_interrupted_download() {
        let data: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let url = range_server(data.clone(), 100_000);
        let dir = std::env::temp_dir().join(format!("scrinium-dl-{}", std::process::id()));
        let dest = dir.join("datei.bin");
        let cancel = AtomicBool::new(false);
        let mut last = Progress::default();
        let first = download_file(&url, &dest, "Test", &mut |p| last = p.clone(), &cancel);
        assert!(first.is_err(), "first connection breaks off");
        assert!(!dest.exists());
        assert_eq!(std::fs::metadata(part_path(&dest)).unwrap().len(), 100_000);
        download_file(&url, &dest, "Test", &mut |p| last = p.clone(), &cancel).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
        assert_eq!(last.total, Some(300_000));
        assert_eq!(last.fraction(), Some(1.0));
        std::fs::remove_dir_all(dir).ok();
    }
}
