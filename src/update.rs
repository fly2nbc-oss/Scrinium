//! Check for a new version (as in Cerno): one request to GitHub for the latest published release. Only when
//! allowed (question on first start) and at most once a day; nothing about the user or their documents is sent,
//! nothing is downloaded or installed.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui;
use serde::{Deserialize, Serialize};

pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const TIMEOUT: Duration = Duration::from_secs(10);
/// The automatic check waits after the start so that it does not slow it down.
const AFTER_START: Duration = Duration::from_secs(5);
/// The question on first start does not appear together with the window.
const ASK_AFTER: Duration = Duration::from_secs(3);
/// Lives next to the configuration.
const FILE: &str = "updates.toml";

/// Version `major.minor.patch`; pre-releases (`-rc1`) and other tags do not count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(u32, u32, u32);

impl Version {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_start_matches('v');
        let mut parts = text.split('.').map(|part| {
            (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())).then(|| part.parse::<u32>().ok()).flatten()
        });
        let version = Self(parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(version)
    }

    pub fn current() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Self(0, 0, 0))
    }

    /// Page of the release on GitHub.
    pub fn page(self) -> String {
        format!("{REPOSITORY}/releases/tag/v{self}")
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Version from the redirect of `/releases/latest` (`…/releases/tag/v0.5.0`).
pub fn version_in(location: &str) -> Option<Version> {
    let (_, tag) = location.rsplit_once("/releases/tag/")?;
    Version::parse(tag.split(['?', '#']).next()?)
}

/// Due: never checked, checked at least a day ago, or the clock was set back.
pub fn due(last_ms: Option<i64>, now_ms: i64) -> bool {
    match last_ms {
        None => true,
        Some(last) => now_ms < last || now_ms - last >= DAY_MS,
    }
}

/// Answer from GitHub.
#[derive(Debug, Clone, PartialEq)]
enum Answer {
    Latest(Version),
    /// No published release (only drafts or pre-releases), or the repository is not public.
    NoRelease,
    Failed(String),
}

/// Asks for the latest release. GitHub redirects `/releases/latest` to the tag; only the redirect is read,
/// without the API and without following it.
fn latest() -> Answer {
    let agent = ureq::AgentBuilder::new().https_only(true).user_agent("Scrinium").redirects(0).timeout(TIMEOUT).build();
    match agent.get(&format!("{REPOSITORY}/releases/latest")).call() {
        Ok(resp) => match resp.header("location") {
            Some(location) => version_in(location).map_or(Answer::NoRelease, Answer::Latest),
            None => Answer::Failed(format!("no redirect ({})", resp.status())),
        },
        Err(ureq::Error::Status(404, _)) => Answer::NoRelease,
        Err(e) => Answer::Failed(e.to_string()),
    }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Saved state in `updates.toml`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    /// Checking allowed; missing until the question on first start is answered.
    check: Option<bool>,
    /// Last successful check (milliseconds since 1970).
    checked_ms: Option<i64>,
    /// Latest version found.
    latest: Option<String>,
    /// Version the notice was already shown for.
    reported: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Never,
    Checking,
    Current,
    Newer(Version),
    NoRelease,
    Failed,
}

pub struct Updates {
    dir: PathBuf,
    saved: Saved,
    pub state: State,
    rx: Option<mpsc::Receiver<Answer>>,
    started: Instant,
    /// The automatic check of this session is done (or was not due).
    looked: bool,
    /// New version that should appear once as a notice.
    pub to_tell: Option<Version>,
}

impl Updates {
    /// Reads the saved state; the version found last applies until the next check.
    pub fn load(dir: &Path) -> Self {
        let saved: Saved =
            std::fs::read_to_string(dir.join(FILE)).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default();
        let state = match saved.latest.as_deref().and_then(Version::parse) {
            Some(v) if v > Version::current() => State::Newer(v),
            Some(_) => State::Current,
            None => State::Never,
        };
        Self { dir: dir.to_path_buf(), saved, state, rx: None, started: Instant::now(), looked: false, to_tell: None }
    }

    fn save(&self) {
        let _ = std::fs::create_dir_all(&self.dir);
        if let Ok(text) = toml::to_string(&self.saved) {
            let _ = std::fs::write(self.dir.join(FILE), text);
        }
    }

    pub fn enabled(&self) -> bool {
        self.saved.check == Some(true)
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.saved.check = Some(on);
        self.save();
    }

    /// Time for the question on first start? If not, a repaint is requested for when it becomes due.
    pub fn should_ask(&self, ctx: &egui::Context) -> bool {
        if self.saved.check.is_some() {
            return false;
        }
        let waited = self.started.elapsed();
        if waited < ASK_AFTER {
            ctx.request_repaint_after(ASK_AFTER - waited);
            return false;
        }
        true
    }

    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    pub fn newer(&self) -> Option<Version> {
        match self.state {
            State::Newer(v) => Some(v),
            _ => None,
        }
    }

    /// Checks now in the background (even when the automatic check is off).
    pub fn check_now(&mut self, ctx: &egui::Context) {
        if self.rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new().name("scrinium-update".into()).spawn(move || {
            let _ = tx.send(latest());
            ctx.request_repaint();
        });
        if spawned.is_ok() {
            self.rx = Some(rx);
            self.state = State::Checking;
        }
    }

    /// Every frame: collect the answer and, if allowed and due, check a few seconds after the start.
    pub fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.rx {
            let answer = match rx.try_recv() {
                Ok(answer) => Some(answer),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Answer::Failed("check aborted".into())),
            };
            if let Some(answer) = answer {
                self.rx = None;
                self.finish(answer);
            }
        }
        if self.looked || !self.enabled() {
            return;
        }
        let waited = self.started.elapsed();
        if waited < AFTER_START {
            ctx.request_repaint_after(AFTER_START - waited);
            return;
        }
        self.looked = true;
        if due(self.saved.checked_ms, now_ms()) {
            self.check_now(ctx);
        }
    }

    fn finish(&mut self, answer: Answer) {
        match answer {
            Answer::Latest(v) => {
                self.saved.checked_ms = Some(now_ms());
                self.saved.latest = Some(v.to_string());
                if v > Version::current() {
                    self.state = State::Newer(v);
                    if self.saved.reported.as_deref().and_then(Version::parse) != Some(v) {
                        self.saved.reported = Some(v.to_string());
                        self.to_tell = Some(v);
                    }
                } else {
                    self.state = State::Current;
                }
                self.save();
            }
            Answer::NoRelease => {
                self.saved.checked_ms = Some(now_ms());
                self.save();
                self.state = State::NoRelease;
            }
            // Offline or proxy: only the status in the info window, no notice. The next session tries again.
            Answer::Failed(_) => self.state = State::Failed,
        }
    }

    /// Status for the info window.
    pub fn status(&self) -> String {
        let t = crate::i18n::general::t();
        let text = match self.state {
            State::Never if !self.enabled() => return t.update_off.to_string(),
            State::Never => t.update_never.to_string(),
            State::Checking => return t.update_checking.to_string(),
            State::Current => t.update_current.to_string(),
            State::Newer(v) => (t.update_newer)(&v.to_string()),
            State::NoRelease => t.update_no_release.to_string(),
            State::Failed => t.update_failed.to_string(),
        };
        if self.enabled() { text } else { format!("{text} {}", t.update_off) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_their_numbers() {
        let v = |s: &str| Version::parse(s).unwrap();
        assert!(v("v0.10.0") > v("0.9.1"));
        assert!(v("1.0.0") > v("0.99.99"));
        assert_eq!(v("v0.5.0"), Version(0, 5, 0));
        for bad in ["0.5", "0.5.0.1", "0.5.0-rc1", "models-1", "", "v", "0..1", "0.x.1"] {
            assert_eq!(Version::parse(bad), None, "{bad}");
        }
        assert_eq!(v("0.5.0").to_string(), "0.5.0");
        assert!(v("0.5.0").page().ends_with("/releases/tag/v0.5.0"));
    }

    #[test]
    fn the_redirect_names_the_release() {
        let base = "https://github.com/fly2nbc-oss/Scrinium";
        assert_eq!(version_in(&format!("{base}/releases/tag/v0.5.0")), Version::parse("0.5.0"));
        assert_eq!(version_in(&format!("{base}/releases/tag/v0.5.0?x=1")), Version::parse("0.5.0"));
        assert_eq!(version_in(&format!("{base}/releases")), None, "no published release");
        assert_eq!(version_in(&format!("{base}/releases/tag/test")), None);
    }

    #[test]
    fn a_check_is_due_once_a_day() {
        assert!(due(None, 1_000));
        assert!(!due(Some(1_000), 1_000 + DAY_MS - 1));
        assert!(due(Some(1_000), 1_000 + DAY_MS));
        assert!(due(Some(5_000), 1_000), "clock set back");
    }

    #[test]
    fn the_saved_state_survives_a_restart() {
        let dir = std::env::temp_dir().join(format!("scrinium-update-test-{}", std::process::id()));
        let mut u = Updates::load(&dir);
        assert_eq!(u.state, State::Never);
        assert!(!u.enabled());
        u.set_enabled(true);
        u.finish(Answer::Latest(Version(99, 0, 0)));
        assert_eq!(u.to_tell, Some(Version(99, 0, 0)), "tell once");
        let again = Updates::load(&dir);
        assert!(again.enabled());
        assert_eq!(again.newer(), Some(Version(99, 0, 0)));
        let mut told = Updates::load(&dir);
        told.finish(Answer::Latest(Version(99, 0, 0)));
        assert_eq!(told.to_tell, None, "not the same version again");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
