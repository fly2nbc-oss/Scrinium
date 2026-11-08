//! Everything that depends on the language of the documents – not of the user interface: words and patterns for
//! dates, doctypes, senders, subjects and legal forms, plus the default list of doctypes. The UI language (`i18n`)
//! and the document language are independent: a German letter stays German when the UI is set to English.
//!
//! Scrinium ships German (`de`). Another language is a new file with its own [`DocLang`].

use std::sync::LazyLock;

use regex::Regex;

mod de;

/// Words and patterns of one document language. Word lists are normalized (lower case, umlauts folded to the base
/// vowel, see `archive::normalize`) unless a field says otherwise.
pub struct DocLang {
    /// Language code as in the UI (`de`).
    #[allow(dead_code)]
    pub code: &'static str,
    /// Day, month name and year ("3. Januar 2025"); groups: day, month name, year.
    pub text_date: LazyLock<Regex>,
    /// Month name and year without a day ("Januar 2016"); groups: month name, year.
    pub month_year: LazyLock<Regex>,
    /// Month number (1–12) from a month name as matched by the patterns above.
    pub month_from_name: fn(&str) -> Option<u32>,
    /// Last word before a letter date ("Musterstadt, den …").
    pub letter_date_words: &'static [&'static str],
    /// Parts of the last word before a letter date ("Datum: …").
    pub letter_date_parts: &'static [&'static str],
    /// Last word before a date that marks a date of the document ("Rechnung vom …", "Stand …").
    pub date_prefix_words: &'static [&'static str],
    /// Words before a date that point to deadlines, periods or birthdays (lower case, not folded).
    pub non_document_words: &'static [&'static str],
    /// Parts of such words (lower case, not folded).
    pub non_document_parts: &'static [&'static str],
    /// Doctype detection: search terms → term of the default list; the order is also the priority.
    pub doctypes: &'static [(&'static [&'static str], &'static str)],
    /// Markers of a company or authority in the letterhead (padded with spaces where needed).
    pub org_markers: &'static [&'static str],
    /// From the street or postcode on, the rest of a letterhead segment belongs to the address.
    pub address_tail: LazyLock<Regex>,
    /// A line that is only a page number ("Seite 2", "Seite 1 von 3").
    pub page_line: LazyLock<Regex>,
    /// Starts of a subject line (lower case).
    pub subject_prefixes: &'static [&'static str],
    /// Legal forms dropped from the short name of a sender (lower case).
    pub legal_forms: &'static [&'static str],
    /// Word stems that lose a linking "s" once the legal form is gone ("Versicherungs-AG" → "Versicherung").
    pub linking_s_stems: &'static [&'static str],
    /// Plural endings tried when matching a doctype against the list.
    pub plural_suffixes: &'static [&'static str],
    /// Common names mapped to a term of the default list.
    pub doctype_synonyms: &'static [(&'static str, &'static str)],
    /// The most general doctype, used when nothing in the list fits (normalized).
    pub general_doctype: &'static str,
    /// Words that negate the next words ("wird nicht unterschrieben").
    pub negations: &'static [&'static str],
    /// Default list of doctypes for a new configuration, singular, by group.
    pub default_doctypes: &'static [&'static str],
}

/// The document language in use. For now always German.
pub fn current() -> &'static DocLang {
    &de::DE
}
