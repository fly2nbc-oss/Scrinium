//! UI languages: German, English, French, Spanish and Italian.
//!
//! As in Cerno, each language is one complete literal; if a text is missing, Scrinium does not build. Because
//! Scrinium has many texts, they are split by area into separate files, each with its own `Texts` struct, the five
//! literals `DE` … `IT` and `t()` for the current language. Texts with values are functions, so each language is
//! free in its word order.
//!
//! The language is global and read again every frame; a change takes effect at once. Text that is already stored
//! (progress of a running analysis, a message) stays in the old language.
//! Not translated are the instructions to the AI, word lists and configuration defaults (such as the document
//! types) and the command line: they determine how files are named, not how the UI speaks.

use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};

pub mod ai;
pub mod analysis;
pub mod files;
pub mod folders;
pub mod form;
pub mod general;
pub mod scan;
pub mod settings;
pub mod setup;
pub mod window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
    Fr,
    Es,
    It,
}

impl Lang {
    pub const ALL: [Lang; 5] = [Self::De, Self::En, Self::Fr, Self::Es, Self::It];

    pub fn code(self) -> &'static str {
        match self {
            Self::De => "de",
            Self::En => "en",
            Self::Fr => "fr",
            Self::Es => "es",
            Self::It => "it",
        }
    }

    /// `de`, `de-DE`, `de_CH.UTF-8` → German.
    pub fn from_code(code: &str) -> Option<Self> {
        let primary = code.split(['-', '_', '.']).next()?.to_ascii_lowercase();
        Self::ALL.into_iter().find(|l| l.code() == primary)
    }

    /// The language's name in that language.
    pub fn name(self) -> &'static str {
        match self {
            Self::De => "Deutsch",
            Self::En => "English",
            Self::Fr => "Français",
            Self::Es => "Español",
            Self::It => "Italiano",
        }
    }
}

/// Defaults to German so tests do not depend on the machine's language; at startup `init` sets the saved one or
/// the system's.
static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Lang {
    Lang::ALL.get(CURRENT.load(Ordering::Relaxed) as usize).copied().unwrap_or(Lang::De)
}

pub fn set(lang: Lang) {
    let index = Lang::ALL.iter().position(|l| *l == lang).unwrap_or(0);
    CURRENT.store(index as u8, Ordering::Relaxed);
}

/// The system language if Scrinium speaks it, otherwise English.
pub fn system_default() -> Lang {
    sys_locale::get_locale().as_deref().and_then(Lang::from_code).unwrap_or(Lang::En)
}

/// At startup: the language saved in `view.toml`, otherwise the system's.
pub fn init(dir: &Path) {
    set(crate::theme::read_view(dir).language.as_deref().and_then(Lang::from_code).unwrap_or_else(system_default));
}

/// Saves the language in `view.toml` and switches to it.
pub fn choose(dir: &Path, lang: Lang) {
    set(lang);
    let mut view = crate::theme::read_view(dir);
    view.language = Some(lang.code().to_string());
    crate::theme::write_view(dir, &view);
}

/// The texts of one area in the current language.
fn pick<T>(de: &'static T, en: &'static T, fr: &'static T, es: &'static T, it: &'static T) -> &'static T {
    match current() {
        Lang::De => de,
        Lang::En => en,
        Lang::Fr => fr,
        Lang::Es => es,
        Lang::It => it,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_locales_find_the_language() {
        for lang in Lang::ALL {
            assert_eq!(Lang::from_code(lang.code()), Some(lang));
        }
        assert_eq!(Lang::from_code("de-DE"), Some(Lang::De));
        assert_eq!(Lang::from_code("fr_CH.UTF-8"), Some(Lang::Fr));
        assert_eq!(Lang::from_code("pt-BR"), None);
    }
}
