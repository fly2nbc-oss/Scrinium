//! Rule-based evaluation of the OCR text. Serves as a fallback without AI and as a starting value that the AI answer
//! refines.

use std::sync::LazyLock;

use chrono::{Datelike, Duration, NaiveDate};
use regex::Regex;

use crate::archive::normalize;
use crate::doclang;
use crate::naming::DocDate;

/// Compiles a pattern from the program code. The patterns are fixed in the code (in `tags.rs` built from names via
/// `regex::escape`); an invalid one would show up in every test run. That is why only this place may abort.
#[allow(clippy::expect_used)]
pub(crate) fn code_regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("pattern in the code is valid")
}

static NUMERIC_DATE: LazyLock<Regex> =
    LazyLock::new(|| code_regex(r"\b(\d{1,2})\s?[./]\s?(\d{1,2})\s?[./]\s?(\d{4}|\d{2})\b"));
static ISO_DATE: LazyLock<Regex> = LazyLock::new(|| code_regex(r"\b(\d{4})-(\d{2})-(\d{2})\b"));
fn expand_year(y: i32) -> i32 {
    match y {
        0..=69 => 2000 + y,
        70..=99 => 1900 + y,
        _ => y,
    }
}

/// How clearly a date in the text is the document date; lower is clearer.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum DateRank {
    /// Letter date: "Musterstadt, den …", "Musterstadt, 15.09.2026", "Datum: …".
    Letter,
    /// "Rechnung vom …", "Kontostand am …", "Stand …".
    Marked,
    /// Without a hint.
    Plain,
    /// Deadline, period or date of birth.
    Excluded,
}

pub struct FoundDate {
    pub date: NaiveDate,
    /// Start of the line up to the date, in lower case.
    pub prefix: String,
}

impl FoundDate {
    fn last_words(&self, n: usize) -> Vec<&str> {
        self.prefix
            .split_whitespace()
            .rev()
            .take(n)
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| !w.is_empty())
            .collect()
    }

    /// Deadline, period or date of birth instead of the document date.
    fn is_excluded(&self) -> bool {
        let lang = doclang::current();
        self.last_words(4)
            .iter()
            .any(|w| lang.non_document_words.contains(w) || lang.non_document_parts.iter().any(|p| w.contains(p)))
    }

    pub fn rank(&self) -> DateRank {
        if self.is_excluded() {
            return DateRank::Excluded;
        }
        let lang = doclang::current();
        let last = self.last_words(1);
        let last = last.first();
        let letter_word =
            |w: &str| lang.letter_date_words.contains(&w) || lang.letter_date_parts.iter().any(|p| w.contains(p));
        if self.prefix.trim_end().ends_with(',') || last.is_some_and(|w| letter_word(w)) {
            DateRank::Letter
        } else if last.is_some_and(|w| lang.date_prefix_words.contains(w)) {
            DateRank::Marked
        } else {
            DateRank::Plain
        }
    }
}

/// The date nearest to today: the latest up to today, otherwise the earliest after it.
pub fn nearest_to_today(dates: impl Iterator<Item = NaiveDate>, today: NaiveDate) -> Option<NaiveDate> {
    let (past, future): (Vec<NaiveDate>, Vec<NaiveDate>) = dates.partition(|d| *d <= today);
    past.into_iter().max().or_else(|| future.into_iter().min())
}

/// The clearest rank among the dates in the text and within it the date nearest to today.
pub fn best_date(found: &[FoundDate], today: NaiveDate) -> Option<(DateRank, NaiveDate)> {
    let rank = found.iter().map(FoundDate::rank).min()?;
    nearest_to_today(found.iter().filter(|f| f.rank() == rank).map(|f| f.date), today).map(|d| (rank, d))
}

/// All plausible dates in reading order.
pub fn find_dates(text: &str, today: NaiveDate) -> Vec<FoundDate> {
    let Some(min) = NaiveDate::from_ymd_opt(1950, 1, 1) else {
        return Vec::new();
    };
    let max = today + Duration::days(60);
    let lang = doclang::current();
    let mut out = Vec::new();
    for line in text.lines() {
        let mut found: Vec<(usize, NaiveDate)> = Vec::new();
        for c in NUMERIC_DATE.captures_iter(line) {
            let (d, m, y) = (c[1].parse().ok(), c[2].parse().ok(), c[3].parse::<i32>().ok().map(expand_year));
            if let (Some(d), Some(m), Some(y)) = (d, m, y)
                && let Some(date) = NaiveDate::from_ymd_opt(y, m, d)
            {
                found.push((c.get(0).map_or(0, |m| m.start()), date));
            }
        }
        for c in ISO_DATE.captures_iter(line) {
            if let (Ok(y), Ok(m), Ok(d)) = (c[1].parse(), c[2].parse(), c[3].parse())
                && let Some(date) = NaiveDate::from_ymd_opt(y, m, d)
            {
                found.push((c.get(0).map_or(0, |m| m.start()), date));
            }
        }
        for c in lang.text_date.captures_iter(line) {
            if let (Ok(d), Some(m), Ok(y)) = (c[1].parse(), (lang.month_from_name)(&c[2]), c[3].parse())
                && let Some(date) = NaiveDate::from_ymd_opt(y, m, d)
            {
                found.push((c.get(0).map_or(0, |m| m.start()), date));
            }
        }
        found.sort_by_key(|(pos, _)| *pos);
        out.extend(
            found
                .into_iter()
                .filter(|(_, d)| *d >= min && *d <= max)
                .map(|(pos, date)| FoundDate { date, prefix: line[..pos].to_lowercase() }),
        );
    }
    out
}

/// Document date: letter dates ("Ort, den …", "Datum: …") before dates with "vom", "am" or "Stand", these before dates
/// without a hint; deadlines and periods last. If several share the same rank, the one nearest to today wins:
/// references to earlier letters and misreadings are older than the letter itself.
pub fn document_date(text: &str, today: NaiveDate) -> Option<NaiveDate> {
    best_date(&find_dates(text, today), today).map(|(_, d)| d)
}

/// Doctype from the head of the document (title, subject), otherwise from the whole text. This way "Kündigungsfrist" in
/// the small print of a contract does not count as a Kündigung.
pub fn doctype(text: &str) -> Option<&'static str> {
    let head: String = text.lines().take(20).collect::<Vec<_>>().join("\n");
    let find = |hay: &str| {
        let hay = normalize(hay);
        doclang::current()
            .doctypes
            .iter()
            .find(|(keys, _)| keys.iter().any(|k| hay.contains(k)))
            .map(|(_, label)| *label)
    };
    find(&head).or_else(|| find(text))
}

static NUMERIC_MONTH_YEAR: LazyLock<Regex> =
    LazyLock::new(|| code_regex(r"\b(0?[1-9]|1[0-2])\s?[/.]\s?((?:19|20)\d{2})\b"));

/// Date without a day ("Abrechnung Januar 2016", "01/2016") if the text has no full date.
pub fn month_year(text: &str, today: NaiveDate) -> Option<DocDate> {
    let max_year = today.year() + 1;
    let ok = |y: i32| (1950..=max_year).contains(&y);
    let lang = doclang::current();
    let named = lang
        .month_year
        .captures_iter(text)
        .find_map(|c| Some(((lang.month_from_name)(&c[1])?, c[2].parse::<i32>().ok().filter(|y| ok(*y))?)));
    let numeric = || {
        NUMERIC_MONTH_YEAR
            .captures_iter(text)
            .find_map(|c| Some((c[1].parse::<u32>().ok()?, c[2].parse::<i32>().ok().filter(|y| ok(*y))?)))
    };
    named.or_else(numeric).and_then(|(m, y)| DocDate::year_month(y, m))
}

/// Date at the start of a file name: `261007_…`, `20260630_…`, `2026-06-30…`, `2025_02_17…`. A hint for documents
/// already named after the scheme.
pub fn filename_date(stem: &str) -> Option<DocDate> {
    static PREFIX: LazyLock<Regex> = LazyLock::new(|| {
        code_regex(r"^(?:(?P<y4>(?:19|20)\d{2})[-_ ]?(?P<m4>\d{2})[-_ ]?(?P<d4>\d{2})|(?P<c>\d{6}))(?:[_\- ]|$)")
    });
    let c = PREFIX.captures(stem.trim())?;
    if let Some(compact) = c.name("c") {
        return DocDate::parse(compact.as_str());
    }
    let y = c.name("y4")?.as_str().parse().ok()?;
    let m = c.name("m4")?.as_str().parse().ok()?;
    let d = c.name("d4")?.as_str().parse().ok()?;
    NaiveDate::from_ymd_opt(y, m, d).map(DocDate::full)
}

/// Letter-spaced text such as "L E I C H T A T H L E T I K" becomes one word again.
pub fn collapse_letter_spacing(text: &str) -> String {
    static SPACED: LazyLock<Regex> = LazyLock::new(|| code_regex(r"\b\p{L}(?: \p{L}){3,}\b"));
    SPACED.replace_all(text, |c: &regex::Captures| c[0].replace(' ', "")).into_owned()
}

/// Separators in the letterhead: "Firma · Straße", "Firma - Straße", "Firma | Ort" or large gaps.
static HEADER_SPLIT: LazyLock<Regex> = LazyLock::new(|| code_regex(r"\s*(?:[·|•,;]|\s[-–—]\s|\s{2,})\s*"));
fn has_org_marker(s: &str) -> bool {
    let padded = format!(" {} ", normalize(s));
    doclang::current().org_markers.iter().any(|m| padded.contains(m))
}

/// Sender from the letterhead: prefers lines with a company or authority marker.
pub fn sender(text: &str) -> Option<String> {
    let head: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).take(15).collect();
    let segments = |line: &str| -> Vec<String> {
        HEADER_SPLIT
            .split(line)
            .map(|seg| doclang::current().address_tail.replace(seg.trim(), "").trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };
    head.iter()
        .find_map(|l| segments(l).into_iter().find(|s| has_org_marker(s)))
        .or_else(|| head.iter().flat_map(|l| segments(l)).find(|s| looks_like_name(s)))
        .map(|s| clean_sender(&s))
        .filter(|s| !s.is_empty())
}

fn looks_like_name(line: &str) -> bool {
    // Page numbers from headers and footers ("Seite 2", "Seite 1 von 3") are not a sender.
    let letters = line.chars().filter(|c| c.is_alphabetic()).count();
    let digits = line.chars().filter(|c| c.is_ascii_digit()).count();
    letters >= 4
        && digits * 3 < letters
        && line.chars().count() <= 50
        && !line.contains('@')
        && !doclang::current().page_line.is_match(line.trim())
}

fn clean_sender(s: &str) -> String {
    let s = s.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != ')');
    s.chars().take(40).collect::<String>().trim().to_string()
}

/// Subject line, if there is one.
pub fn subject(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find_map(|l| {
            let lower = l.to_lowercase();
            doclang::current()
                .subject_prefixes
                .iter()
                .find_map(|p| lower.starts_with(p).then(|| l[p.len()..].trim().chars().take(60).collect::<String>()))
        })
        .filter(|s| !s.is_empty())
}

/// Short description from the first meaningful lines.
pub fn summary(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines().map(str::trim).filter(|l| l.split_whitespace().count() >= 4) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(line);
        if out.chars().count() > 160 {
            break;
        }
    }
    let mut s: String = out.chars().take(200).collect();
    if out.chars().count() > 200 {
        s.push('…');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 10).unwrap()
    }

    const LETTER: &str = "Stadtwerke Musterstadt GmbH · Werkstraße 2 · 12345 Musterstadt\n\
        Herrn Max Mustermann\nHauptstraße 1\n12345 Musterstadt\n\
        Kundennummer 4711                     Musterstadt, den 12. März 2024\n\
        Betreff: Jahresabrechnung Strom 2023\n\
        Sehr geehrter Herr Mustermann, anbei erhalten Sie Ihre Rechnung für den Zeitraum 01.01.2023 bis 31.12.2023.\n\
        Bitte überweisen Sie den Betrag bis zum 30.04.2024.";

    #[test]
    fn date_formats() {
        let d = |s: &str| document_date(s, today());
        assert_eq!(d("Datum: 05.06.2023"), NaiveDate::from_ymd_opt(2023, 6, 5));
        assert_eq!(d("am 5.6.23 erstellt"), NaiveDate::from_ymd_opt(2023, 6, 5));
        assert_eq!(d("2022-11-30"), NaiveDate::from_ymd_opt(2022, 11, 30));
        assert_eq!(d("Berlin, 3. Januar 2025"), NaiveDate::from_ymd_opt(2025, 1, 3));
        assert_eq!(d("1. Mrz. 2021"), NaiveDate::from_ymd_opt(2021, 3, 1));
        assert_eq!(d("31.02.2024 und 99.99.9999"), None);
        assert_eq!(d("Termin 01.01.2099"), None, "far in the future");
    }

    #[test]
    fn letter_date_beats_period_and_due_date() {
        assert_eq!(document_date(LETTER, today()), NaiveDate::from_ymd_opt(2024, 3, 12));
        let ocr_without_umlauts = "Bitte uberweisen Sie den Betrag bis zum 30.04.2024.\nMusterstadt, den 12. M?rz 2024";
        assert_eq!(document_date(ocr_without_umlauts, today()), NaiveDate::from_ymd_opt(2024, 3, 12));
        let due_first = "Zahlbar bis 01.02.2024\nJahresbeitrag ab 01.01.2024\nKundennr. 5 vom 15.01.2024";
        assert_eq!(document_date(due_first, today()), NaiveDate::from_ymd_opt(2024, 1, 15));
    }

    #[test]
    fn several_dates_the_one_nearest_today() {
        let d = |s: &str| document_date(s, today());
        // Misreading at the top ("07.06.21)25" instead of 2025), the letter date is further down.
        let misread = "Kontostand in EUR am 07.06.21)25, Auszug 1\nMusterstadt, den 15.09.2025";
        assert_eq!(d(misread), NaiveDate::from_ymd_opt(2025, 9, 15));
        let replies = "Ihr Schreiben vom 01.02.2024\nunsere Mahnung vom 05.03.2024";
        assert_eq!(d(replies), NaiveDate::from_ymd_opt(2024, 3, 5), "the newer letter");
        assert_eq!(d("07.01.2025\nRechnung\n09.01.2025"), NaiveDate::from_ymd_opt(2025, 1, 9));
        // A later premium date does not count against the letter date.
        let premium = "Musterstadt, 15.09.2026\nFolgebeitrag am 01.10.2026 99,00 EUR";
        assert_eq!(d(premium), NaiveDate::from_ymd_opt(2026, 9, 15));
        // If all dates are after today, the earliest.
        assert_eq!(d("Termin 20.10.2026\nTermin 30.10.2026"), NaiveDate::from_ymd_opt(2026, 10, 20));
        assert_eq!(d("Geb. 09.02.1977\nUlm, 03.10.2026"), NaiveDate::from_ymd_opt(2026, 10, 3));
    }

    #[test]
    fn partial_and_filename_dates() {
        assert_eq!(month_year("Abrechnung für Januar 2016", today()), DocDate::year_month(2016, 1));
        assert_eq!(month_year("Zeitraum 03/2024", today()), DocDate::year_month(2024, 3));
        assert_eq!(month_year("Seite 1/2", today()), None);
        assert_eq!(filename_date("261007_Urkunde").map(DocDate::display).as_deref(), Some("2026-10-07"));
        assert_eq!(filename_date("160100_Gehalt").map(DocDate::display).as_deref(), Some("2016-01"));
        assert_eq!(filename_date("20260630_Kontoauszug").map(DocDate::display).as_deref(), Some("2026-06-30"));
        assert_eq!(filename_date("2025_02_17_Bescheid").map(DocDate::display).as_deref(), Some("2025-02-17"));
        assert_eq!(filename_date("Urkunde-1"), None);
        assert_eq!(filename_date("1234567890-ER01"), None);
    }

    #[test]
    fn letter_spacing_is_collapsed() {
        assert_eq!(
            collapse_letter_spacing("L E I C H T A T H L E T I K\nU R K U N D E 2026"),
            "LEICHTATHLETIK\nURKUNDE 2026"
        );
        assert_eq!(collapse_letter_spacing("a b c"), "a b c");
    }

    #[test]
    fn detects_type_sender_subject() {
        assert_eq!(doctype(LETTER), Some("Rechnung"));
        assert_eq!(
            doctype("Mitgliedsvertrag\nFitnessstudio\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\nKündigungsfrist 3 Monate"),
            Some("Mitgliedsvertrag")
        );
        assert_eq!(doctype("Bestätigung Ihrer Kündigung"), Some("Kündigungsbestätigung"));
        assert_eq!(doctype("Brutto-Netto-Abrechnung 2026"), Some("Gehaltsabrechnung"));
        assert_eq!(doctype("URKUNDE Stadtlauf Musterstadt"), Some("Urkunde"));
        assert_eq!(doctype("ZAHLUNGSERINNERUNG zu Rechnung 123"), Some("Mahnung"));
        assert_eq!(doctype("Ihre Kundigung"), Some("Kündigung"));
        assert_eq!(sender(LETTER).as_deref(), Some("Stadtwerke Musterstadt GmbH"));
        for (head, expected) in [
            ("Stadtwerke Musterstadt GmbH - Werkstraße 2 - 12345 Musterstadt", "Stadtwerke Musterstadt GmbH"),
            ("Finanzamt Musterstadt Steuerstra?e  12345 Musterstadt", "Finanzamt Musterstadt"),
            ("Stadtwerke Musterstadt GmbH Werkstr, 12345", "Stadtwerke Musterstadt GmbH"),
            ("Exempla Versicherungs-AG - Lindenstraße 28 - 12345 Musterstadt", "Exempla Versicherungs-AG"),
            ("Dr. med. Anna Weber\nPraxis für Allgemeinmedizin", "Praxis für Allgemeinmedizin"),
            ("Seite 2\n\nErika Muster\nHauptstr. 1", "Erika Muster"),
            ("Seite 1 von 3\nMax Muster", "Max Muster"),
        ] {
            assert_eq!(sender(head).as_deref(), Some(expected), "{head}");
        }
        assert_eq!(subject(LETTER).as_deref(), Some("Jahresabrechnung Strom 2023"));
        assert!(summary(LETTER).contains("Jahresabrechnung"));
    }
}
