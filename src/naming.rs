//! File names after the filing scheme: document date (also incomplete), name parts, character rules and uniform
//! spellings for person, sender and doctype.

use crate::doclang;
use std::fmt::Write as _;

use chrono::{Datelike, NaiveDate};

use crate::archive::normalize;
use crate::config::Alias;

/// Date as it appears on the document. If the day or month is missing, nothing is made up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocDate {
    pub year: i32,
    pub month: Option<u32>,
    pub day: Option<u32>,
}

impl DocDate {
    pub fn full(d: NaiveDate) -> Self {
        Self { year: d.year(), month: Some(d.month()), day: Some(d.day()) }
    }

    pub fn year_month(year: i32, month: u32) -> Option<Self> {
        (1..=12).contains(&month).then_some(Self { year, month: Some(month), day: None })
    }

    pub fn year_only(year: i32) -> Self {
        Self { year, month: None, day: None }
    }

    /// Full date, otherwise `None`.
    pub fn to_naive(self) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(self.year, self.month?, self.day?)
    }

    pub fn is_full(self) -> bool {
        self.to_naive().is_some()
    }

    /// Display in the form: `2026-10-07`, `2026-10` or `2026`.
    pub fn display(self) -> String {
        match (self.month, self.day) {
            (Some(m), Some(d)) => format!("{:04}-{m:02}-{d:02}", self.year),
            (Some(m), None) => format!("{:04}-{m:02}", self.year),
            _ => format!("{:04}", self.year),
        }
    }

    /// Reads `2026-10-07`, `07.10.2026`, `7.10.26`, `2026-10`, `10.2026`, `10/2026`, `2026` and the filing format
    /// `261007` (with `00` for unknown parts, e.g. `160100`).
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        // Two-digit years first, otherwise %Y reads "26" as the year 26.
        for f in ["%Y-%m-%d", "%d.%m.%y", "%d.%m.%Y", "%Y/%m/%d", "%Y%m%d"] {
            if let Ok(d) = NaiveDate::parse_from_str(s, f)
                && d.year() >= 1000
            {
                return Some(Self::full(d));
            }
        }
        let digits = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit());
        let parts: Vec<&str> = s.split(['-', '.', '/']).map(str::trim).collect();
        match parts.as_slice() {
            [y] if y.len() == 4 && digits(y) => Some(Self::year_only(y.parse().ok()?)),
            [y] if y.len() == 6 && digits(y) => Self::from_compact(y),
            [y, m] if y.len() == 4 && digits(y) && digits(m) => Self::year_month(y.parse().ok()?, m.parse().ok()?),
            [m, y] if y.len() == 4 && digits(y) && digits(m) => Self::year_month(y.parse().ok()?, m.parse().ok()?),
            _ => None,
        }
    }

    /// `YYMMDD` with `00` for an unknown day or month; `000000` gives `None`.
    fn from_compact(s: &str) -> Option<Self> {
        let n = |r: std::ops::Range<usize>| s.get(r).and_then(|p| p.parse::<u32>().ok());
        let (yy, mm, dd) = (n(0..2)?, n(2..4)?, n(4..6)?);
        if yy == 0 && mm == 0 && dd == 0 {
            return None;
        }
        let year = expand_two_digit_year(yy as i32);
        match (mm, dd) {
            (0, 0) => Some(Self::year_only(year)),
            (m, 0) => Self::year_month(year, m),
            (m, d) => NaiveDate::from_ymd_opt(year, m, d).map(Self::full),
        }
    }
}

/// Two-digit years: 00–69 → 2000s, 70–99 → 1900s.
pub fn expand_two_digit_year(y: i32) -> i32 {
    match y {
        0..=69 => 2000 + y,
        70..=99 => 1900 + y,
        _ => y,
    }
}

/// Formats a (possibly incomplete) date. An unknown day or month becomes `00`: `261007`, `160100`, `160000`. Without a
/// date `{date}` stays empty and is dropped from the file name together with its separator.
pub fn format_date(date: Option<DocDate>, fmt: &str) -> String {
    if date.is_none() {
        return String::new();
    }
    if let Some(full) = date.and_then(DocDate::to_naive) {
        let mut s = String::new();
        if write!(s, "{}", full.format(fmt)).is_ok() && !s.is_empty() {
            return s;
        }
        return full.format("%Y-%m-%d").to_string();
    }
    let mut out = String::new();
    let mut chars = fmt.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('Y') => out.push_str(&date.map(|d| format!("{:04}", d.year)).unwrap_or_else(|| "0000".into())),
            Some('y') => {
                out.push_str(&date.map(|d| format!("{:02}", d.year.rem_euclid(100))).unwrap_or_else(|| "00".into()))
            }
            Some('m') => {
                out.push_str(&date.and_then(|d| d.month).map(|m| format!("{m:02}")).unwrap_or_else(|| "00".into()))
            }
            Some('d') => out.push_str("00"),
            Some('%') => out.push('%'),
            _ => {}
        }
    }
    out
}

/// Fields inserted into file name templates.
#[derive(Debug, Clone, Default)]
pub struct TemplateFields<'a> {
    pub date: Option<DocDate>,
    pub doctype: &'a str,
    pub sender: &'a str,
    pub subject: &'a str,
    pub person: &'a str,
    pub extra: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct NameRules {
    /// Character rules of the scheme: space → `-`, only letters, digits, `-` and `_`.
    pub compact: bool,
    pub max_len: usize,
}

impl Default for NameRules {
    fn default() -> Self {
        Self { compact: true, max_len: 80 }
    }
}

/// Inserts the name parts into the template. Empty parts are dropped together with their separator. If the name is too
/// long, the subject is shortened first, then the sender.
pub fn render_filename(template: &str, date_format: &str, f: &TemplateFields, rules: NameRules) -> String {
    let clean = |s: &str| if rules.compact { compact_component(s) } else { crate::archive::sanitize_component(s) };
    let date = format_date(f.date, date_format);
    let (doctype, sender, person, extra) = (clean(f.doctype), clean(f.sender), clean(f.person), clean(f.extra));
    let mut subject_words: Vec<String> =
        clean(f.subject).split('-').filter(|w| !w.is_empty()).map(String::from).collect();
    // If the doctype is already in the name, it is dropped from the subject (not `Rechnung_…_Rechnung-Bremsen`).
    if template.contains("{doctype}") {
        remove_phrase(&mut subject_words, &doctype);
    }
    let mut sender_words: Vec<String> = sender.split('-').filter(|w| !w.is_empty()).map(String::from).collect();
    let render = |subject: &str, sender: &str| -> String {
        let (year, month, day) = match f.date {
            Some(d) => (
                format!("{:04}", d.year),
                d.month.map(|m| format!("{m:02}")).unwrap_or_else(|| "00".into()),
                d.day.map(|x| format!("{x:02}")).unwrap_or_else(|| "00".into()),
            ),
            None => Default::default(),
        };
        let raw = template
            .replace("{date}", &date)
            .replace("{year}", &year)
            .replace("{month}", &month)
            .replace("{day}", &day)
            .replace("{doctype}", &doctype)
            .replace("{sender}", sender)
            .replace("{subject}", subject)
            .replace("{title}", subject)
            .replace("{person}", &person)
            .replace("{extra}", &extra);
        // Also cleans literal text in the template (`/` or `:` must not get into the name).
        let cleaned = if rules.compact {
            cleanup_separators(&crate::archive::sanitize_component(&raw).replace(' ', "-"))
        } else {
            cleanup_separators(&crate::archive::sanitize_component(&raw))
        };
        if cleaned.is_empty() { "Dokument".to_string() } else { cleaned }
    };
    let mut name = render(&subject_words.join("-"), &sender_words.join("-"));
    let max = rules.max_len.max(20);
    while name.chars().count() > max {
        if subject_words.pop().is_none() {
            if sender_words.len() <= 1 {
                break;
            }
            sender_words.pop();
        }
        name = render(&subject_words.join("-"), &sender_words.join("-"));
    }
    if name.chars().count() > max {
        name = name.chars().take(max).collect::<String>();
        name = cleanup_separators(&name);
    }
    avoid_reserved_name(name)
}

/// Removes every occurrence of the word sequence `phrase` (words joined with `-`) from `words`, ignoring case. Whole
/// words only: "Stromrechnung" stays for "Rechnung".
fn remove_phrase(words: &mut Vec<String>, phrase: &str) {
    let phrase: Vec<String> = phrase.split('-').filter(|w| !w.is_empty()).map(str::to_lowercase).collect();
    if phrase.is_empty() {
        return;
    }
    let mut i = 0;
    while i + phrase.len() <= words.len() {
        if words[i..i + phrase.len()].iter().zip(&phrase).all(|(w, p)| w.to_lowercase() == *p) {
            words.drain(i..i + phrase.len());
        } else {
            i += 1;
        }
    }
}

/// A name part after the character rules: words joined with `-`, no spaces, no special characters such as `& + # ( ) ,
/// ; ' ! ?` and no dots.
pub fn compact_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if c.is_whitespace() || matches!(c, '-' | '_' | '/' | '\\' | ',' | ';' | ':' | '|' | '–' | '—') {
            out.push('-');
        }
    }
    let mut collapsed = String::with_capacity(out.len());
    for c in out.chars() {
        if c == '-' && collapsed.ends_with('-') {
            continue;
        }
        collapsed.push(c);
    }
    collapsed.trim_matches('-').to_string()
}

/// Cleans up separators left by empty name parts (`261007__Rechnung_` → `261007_Rechnung`).
pub fn cleanup_separators(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev: Option<char> = None;
    for c in s.chars() {
        let sep = matches!(c, '_' | ' ' | '-');
        if sep && prev.is_some_and(|p| matches!(p, '_' | ' ' | '-')) {
            // `_` takes precedence over `-` and spaces.
            if c == '_' {
                out.pop();
                out.push('_');
                prev = Some('_');
            }
            continue;
        }
        out.push(c);
        prev = Some(c);
    }
    out.trim_matches(|c: char| c == '_' || c == '-' || c == ' ' || c == '.').to_string()
}

fn avoid_reserved_name(s: String) -> String {
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
        "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&s.to_uppercase().as_str()) { format!("{s}_") } else { s }
}

/// Year the document refers to: a period at the start of the subject (`2025`, `2026-01`, `2026-Q1`, the tax year for
/// taxes), otherwise the year of the document date.
pub fn reference_year(subject: &str, date: Option<DocDate>) -> Option<i32> {
    let s = subject.trim();
    let head: String = s.chars().take(4).collect();
    let next = s.chars().nth(4);
    let is_year = head.len() == 4
        && head.chars().all(|c| c.is_ascii_digit())
        && (head.starts_with("19") || head.starts_with("20"))
        && next.is_none_or(|c| !c.is_alphanumeric() || c == 'Q');
    if is_year { head.parse().ok() } else { date.map(|d| d.year) }
}

/// A period as subject, uniformly as `YYYY-MM`: "September 2026", "Sept. 2026", "09/2026", "01.07.2026-31.07.2026"
/// (exactly one month). Everything else stays as it is.
pub fn normalize_period(subject: &str) -> String {
    let s = subject.trim();
    match period_month(s) {
        Some((y, m)) => format!("{y}-{m:02}"),
        None => s.to_string(),
    }
}

fn period_month(s: &str) -> Option<(i32, u32)> {
    let parts: Vec<&str> = s.split([' ', '-', '/', '.', '_', '–']).filter(|p| !p.is_empty()).collect();
    let year = |p: &str| -> Option<i32> {
        (p.len() == 4 && (p.starts_with("19") || p.starts_with("20"))).then(|| p.parse().ok()).flatten()
    };
    let number = |p: &str, max: u32| p.parse::<u32>().ok().filter(|n| (1..=max).contains(n));
    let whole_month = |(d1, m1, y1): (u32, u32, i32), (d2, m2, y2): (u32, u32, i32)| {
        let last = NaiveDate::from_ymd_opt(y1, m1, 1)?.checked_add_months(chrono::Months::new(1))?.pred_opt()?;
        (d1 == 1 && (m1, y1) == (m2, y2) && d2 == last.day()).then_some((y1, m1))
    };
    // DDMMYYYY as one piece: "01072026-31072026"
    let compact = |p: &str| -> Option<(u32, u32, i32)> {
        (p.len() == 8 && p.chars().all(|c| c.is_ascii_digit()))
            .then(|| Some((number(&p[..2], 31)?, number(&p[2..4], 12)?, year(&p[4..])?)))
            .flatten()
    };
    match parts.as_slice() {
        [a, b] if compact(a).is_some() => whole_month(compact(a)?, compact(b)?),
        [m, y] => Some((year(y)?, month_name(m).or_else(|| number(m, 12))?)),
        [d1, m1, y1, d2, m2, y2] => {
            whole_month((number(d1, 31)?, number(m1, 12)?, year(y1)?), (number(d2, 31)?, number(m2, 12)?, year(y2)?))
        }
        _ => None,
    }
}

fn month_name(word: &str) -> Option<u32> {
    let w = normalize(word);
    let months = [
        &["januar", "janner", "jan"][..],
        &["februar", "feb"],
        &["marz", "maerz", "mrz", "mar"],
        &["april", "apr"],
        &["mai"],
        &["juni", "jun"],
        &["juli", "jul"],
        &["august", "aug"],
        &["september", "sept", "sep"],
        &["oktober", "okt"],
        &["november", "nov"],
        &["dezember", "dez"],
    ];
    months.iter().position(|names| names.contains(&w.as_str())).map(|i| i as u32 + 1)
}

fn split_words(s: &str) -> Vec<String> {
    s.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '/' | '&' | '+' | '_' | '-'))
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| !w.is_empty() && !matches!(normalize(w).as_str(), "und" | "and"))
        .collect()
}

/// Replaces every spelling of a person in running text with their name (`Maximilian` → `Max`).
pub fn canonical_names(text: &str, persons: &[Alias]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if !word.is_empty() {
            let n = normalize(word);
            match persons.iter().find(|p| p.matches_word(&n)) {
                Some(p) => out.push_str(&p.name),
                None => out.push_str(word),
            }
            word.clear();
        }
    };
    for c in text.chars() {
        if c.is_alphanumeric() {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// Uniform spelling of the person(s): aliases become the name (`Maximilian` → `Max`), surnames are dropped, several are
/// joined with `-`, all together are called `Familie`. Unknown persons (relatives) stay as `Firstname-Lastname`.
pub fn normalize_person(raw: &str, persons: &[Alias]) -> String {
    let raw = raw.trim();
    if raw.is_empty() || persons.is_empty() {
        return compact_component(raw);
    }
    if normalize(raw) == "familie" {
        return "Familie".into();
    }
    let words = split_words(raw);
    let mut hit = vec![false; persons.len()];
    for w in &words {
        let w = normalize(w);
        if w == "familie" {
            return "Familie".into();
        }
        if let Some(i) = persons.iter().position(|p| p.matches_word(&w)) {
            hit[i] = true;
        }
    }
    let found: Vec<&str> = persons.iter().zip(&hit).filter(|(_, h)| **h).map(|(p, _)| p.name.as_str()).collect();
    match found.len() {
        0 => compact_component(raw),
        n if n == persons.len() && n > 1 => "Familie".into(),
        _ => found.join("-"),
    }
}

/// Keeps of the (already normalized) person only those who appear in the folder path, also via an alias: `Max-Erika` in
/// `Gehalt/2026_Max` → `Max`, in `Bank/2026` → empty.
pub fn person_in_folder(person: &str, folder: &str, persons: &[Alias]) -> String {
    let words: Vec<String> = split_words(folder).iter().map(|w| normalize(w)).collect();
    let in_folder = |name: &str| {
        let n = normalize(name);
        match persons.iter().find(|p| normalize(&p.name) == n) {
            Some(p) => words.iter().any(|w| p.matches_word(w)),
            None => words.contains(&n),
        }
    };
    person.split('-').filter(|p| !p.is_empty() && in_folder(p)).collect::<Vec<_>>().join("-")
}

/// Removes legal forms such as `GmbH`, `AG`, `& Co. KG`, `e.V.` (also when attached as in `Versicherungs-AG`).
pub fn strip_legal_form(s: &str) -> String {
    let mut out = s.trim().to_string();
    let legal_form_start = |out: &str| {
        doclang::current().legal_forms.iter().find_map(|form| {
            let pos = out.len().checked_sub(form.len())?;
            if !out.is_char_boundary(pos) || out[pos..].to_lowercase() != *form {
                return None;
            }
            out[..pos].chars().last().is_some_and(|c| c == ' ' || c == '-' || c == ',').then_some(pos)
        })
    };
    while let Some(cut) = legal_form_start(&out) {
        out = out[..cut].trim_end_matches([' ', '-', ',']).to_string();
    }
    // "Exempla Versicherungs-AG" → "Exempla Versicherung"
    let lower = out.to_lowercase();
    if doclang::current().linking_s_stems.iter().any(|stem| lower.ends_with(stem)) {
        out.pop();
    }
    if out.is_empty() { s.trim().to_string() } else { out }
}

/// Short name of the sender: legal form removed, known aliases mapped to the everyday name (`M+V` → `MuV`).
pub fn normalize_sender(raw: &str, senders: &[Alias]) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    let hay = normalize(raw);
    let best = senders
        .iter()
        .flat_map(|s| std::iter::once(&s.name).chain(&s.aliases).map(move |a| (s, normalize(a.trim()))))
        .filter(|(_, a)| !a.is_empty() && contains_term(&hay, a))
        .max_by_key(|(_, a)| a.chars().count());
    match best {
        Some((alias, _)) => alias.name.clone(),
        None => soften_capitals(&strip_legal_form(raw)),
    }
}

/// Words in all capitals (longer than 4 letters) get normal case.
fn soften_capitals(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let letters: Vec<char> = w.chars().filter(|c| c.is_alphabetic()).collect();
            if letters.len() > 4 && letters.iter().all(|c| c.is_uppercase()) {
                let mut chars = w.chars();
                chars.next().map(|f| f.to_string() + &chars.as_str().to_lowercase()).unwrap_or_default()
            } else {
                w.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Substring match; short terms (up to 3 characters, e.g. `TK`) only as a whole word.
fn contains_term(hay: &str, term: &str) -> bool {
    if term.chars().count() > 3 {
        return hay.contains(term);
    }
    hay.match_indices(term).any(|(i, _)| {
        let before = hay[..i].chars().last();
        let after = hay[i + term.len()..].chars().next();
        before.is_none_or(|c| !c.is_alphanumeric()) && after.is_none_or(|c| !c.is_alphanumeric())
    })
}

/// Maps a doctype onto the fixed list. If nothing fits, the more general term `Schreiben` applies (if in the list).
/// With an empty list the term stays as it is.
pub fn canonical_doctype(raw: &str, list: &[String]) -> String {
    let raw = raw.trim();
    if list.is_empty() || raw.is_empty() {
        return raw.to_string();
    }
    if let Some(d) = same_doctype(raw, list) {
        return d;
    }
    let n = normalize(raw);
    let find = |wanted: &str| list.iter().find(|d| normalize(d) == wanted).cloned();
    // "Wohngebäude-Kündigungsbestätigung" → "Kündigungsbestätigung": longest matching ending.
    if let Some(d) = list.iter().filter(|d| n.ends_with(&normalize(d))).max_by_key(|d| d.chars().count()) {
        return d.clone();
    }
    find(doclang::current().general_doctype).unwrap_or_else(|| raw.to_string())
}

/// The same term in the list: spelled the same, in the plural or as a known synonym.
fn same_doctype(raw: &str, list: &[String]) -> Option<String> {
    let n = normalize(raw.trim());
    let find = |wanted: &str| list.iter().find(|d| normalize(d) == wanted).cloned();
    if let Some(d) = find(&n) {
        return Some(d);
    }
    for suffix in doclang::current().plural_suffixes {
        if let Some(d) = n.strip_suffix(suffix).and_then(find) {
            return Some(d);
        }
    }
    doclang::current()
        .doctype_synonyms
        .iter()
        .find(|(k, _)| n == *k || n.ends_with(k))
        .and_then(|(_, target)| find(&normalize(target)))
}

/// The AI's own doctype because no term of the list fits. If it is in the list after all (also in the plural or as a
/// synonym), the list term applies; otherwise it stays – without falling back to a more general term such as
/// "Bescheinigung" or "Schreiben".
pub fn own_doctype(raw: &str, list: &[String]) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    if let Some(d) = same_doctype(raw, list) {
        return d;
    }
    let mut chars = raw.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// `specific` is a more specific kind of `general` (compound with the same head word): `Gebührenbescheid` for
/// `Bescheid`, `Kündigungsbestätigung` for `Bestätigung`. A list entry in between that is not a compound itself
/// separates them: `Gehaltsabrechnung` is an `Abrechnung`, not a `Rechnung`.
pub fn refines(specific: &str, general: &str, list: &[String]) -> bool {
    // At least three characters of modifier, so "Ab-rechnung" does not count as "Rechnung".
    let compound = |s: &str, g: &str| !g.is_empty() && s.len() >= g.len() + 3 && s.ends_with(g);
    let (s, g) = (normalize(specific), normalize(general));
    compound(&s, &g)
        && !list
            .iter()
            .map(|m| normalize(m))
            .any(|m| m != s && m != g && s.ends_with(&m) && m.ends_with(&g) && !compound(&m, &g))
}

/// General terms of the list with their more specific forms, e.g. `Bescheid` → `Steuerbescheid`, … Terms that are a
/// more specific form themselves appear only under their general term.
pub fn doctype_families(list: &[String]) -> Vec<(&str, Vec<&str>)> {
    list.iter()
        .filter(|g| !list.iter().any(|p| refines(g, p, list)))
        .filter_map(|g| {
            let specific: Vec<&str> = list.iter().filter(|s| refines(s, g, list)).map(String::as_str).collect();
            (!specific.is_empty()).then_some((g.as_str(), specific))
        })
        .collect()
}

/// The extension belongs to a program file whose name is never changed (`*` at the end = prefix).
pub fn keeps_name(ext: &str, patterns: &[String]) -> bool {
    let ext = ext.to_lowercase();
    patterns.iter().map(|p| p.trim().trim_start_matches('.').to_lowercase()).any(|p| match p.strip_suffix('*') {
        Some(prefix) => !prefix.is_empty() && ext.starts_with(prefix),
        None => !p.is_empty() && ext == p,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> Option<DocDate> {
        NaiveDate::from_ymd_opt(y, m, day).map(DocDate::full)
    }

    fn persons() -> Vec<Alias> {
        vec![
            Alias { name: "Max".into(), aliases: vec!["Maximilian".into(), "MM".into()] },
            Alias { name: "Erika".into(), aliases: vec!["EM".into()] },
            Alias { name: "Paul".into(), aliases: vec![] },
            Alias { name: "Lena".into(), aliases: vec![] },
        ]
    }

    const T: &str = "{date}_{doctype}_{sender}_{subject}_{person}_{extra}";

    fn name(f: TemplateFields) -> String {
        render_filename(T, "%y%m%d", &f, NameRules::default())
    }

    #[test]
    fn own_doctype_is_not_bent_to_the_list() {
        let list: Vec<String> = ["Bescheinigung", "Schreiben", "Mahnung"].map(String::from).to_vec();
        assert_eq!(own_doctype("Meldebescheinigung", &list), "Meldebescheinigung", "not the closest match");
        assert_eq!(canonical_doctype("Meldebescheinigung", &list), "Bescheinigung", "list choice as before");
        assert_eq!(own_doctype("Mahnungen", &list), "Mahnung", "is in the list after all");
        assert_eq!(own_doctype("mietvertrag", &list), "Mietvertrag");
        assert_eq!(own_doctype("  ", &list), "");
    }

    #[test]
    fn dates_with_unknown_parts() {
        assert_eq!(format_date(d(2026, 9, 25), "%y%m%d"), "260925");
        assert_eq!(format_date(DocDate::year_month(2016, 1), "%y%m%d"), "160100");
        assert_eq!(format_date(Some(DocDate::year_only(2016)), "%y%m%d"), "160000");
        assert_eq!(format_date(None, "%y%m%d"), "", "without a date {{date}} stays empty");
        assert_eq!(format_date(None, "%Y-%m-%d"), "");
        for (input, shown) in [
            ("2026-10-07", "2026-10-07"),
            ("07.10.2026", "2026-10-07"),
            ("7.10.26", "2026-10-07"),
            ("2016-01", "2016-01"),
            ("01.2016", "2016-01"),
            ("2016", "2016"),
            ("160100", "2016-01"),
            ("261007", "2026-10-07"),
        ] {
            assert_eq!(DocDate::parse(input).map(DocDate::display).as_deref(), Some(shown), "{input}");
        }
        assert_eq!(DocDate::parse("000000"), None);
        assert_eq!(DocDate::parse("13.2016"), None);
        assert_eq!(DocDate::parse("31.02.2024"), None);
    }

    #[test]
    fn schema_examples_from_the_rules() {
        let f = |date, doctype, sender, subject, person| TemplateFields {
            date,
            doctype,
            sender,
            subject,
            person,
            extra: "",
        };
        assert_eq!(
            name(f(d(2026, 9, 25), "Steuerbescheid", "Finanzamt", "2025", "")),
            "260925_Steuerbescheid_Finanzamt_2025"
        );
        assert_eq!(
            name(f(d(2026, 4, 15), "Rechnung", "Autohaus-Muster", "320d Bremsen", "")),
            "260415_Rechnung_Autohaus-Muster_320d-Bremsen"
        );
        assert_eq!(
            name(f(d(2026, 7, 21), "Rechnung", "Zahnarzt Weber Klein", "", "Max")),
            "260721_Rechnung_Zahnarzt-Weber-Klein_Max"
        );
        assert_eq!(
            name(f(d(2026, 1, 30), "Gehaltsabrechnung", "Acme", "2026-01", "Max")),
            "260130_Gehaltsabrechnung_Acme_2026-01_Max"
        );
        assert_eq!(
            name(f(None, "Mitteilung", "BZSt", "Steuer-ID", "Max")),
            "Mitteilung_BZSt_Steuer-ID_Max",
            "an unknown date is left out, not made up"
        );
        assert_eq!(
            render_filename(
                "{sender}_{date}_{doctype}",
                "%Y-%m-%d",
                &f(None, "Rechnung", "SWM", "", ""),
                NameRules::default()
            ),
            "SWM_Rechnung",
            "also in the middle of the template"
        );
        assert_eq!(
            name(f(d(2026, 4, 15), "Rechnung", "Autohaus-Muster", "Rechnung Bremsen BMW", "")),
            "260415_Rechnung_Autohaus-Muster_Bremsen-BMW",
            "doctype not twice"
        );
        assert_eq!(
            name(f(d(2026, 3, 5), "Kündigungsbestätigung", "VGM", "kündigungsbestätigung", "")),
            "260305_Kündigungsbestätigung_VGM"
        );
        assert_eq!(
            name(f(d(2025, 7, 1), "Rechnung", "EnMS", "Stromrechnung 2025", "")),
            "250701_Rechnung_EnMS_Stromrechnung-2025",
            "whole words only"
        );
        assert_eq!(
            render_filename(
                "{date}_{subject}",
                "%y%m%d",
                &f(d(2025, 7, 1), "Rechnung", "", "Rechnung 7", ""),
                NameRules::default()
            ),
            "250701_Rechnung-7",
            "without {{doctype}} in the template the subject stays"
        );
        let signed = TemplateFields { extra: "unterschrieben", ..f(d(2022, 12, 13), "Antrag", "HKM", "Kfz 320d", "") };
        assert_eq!(name(signed), "221213_Antrag_HKM_Kfz-320d_unterschrieben");
        assert_eq!(
            name(f(d(2025, 3, 14), "Rechnung", "Schröder", "Fernbedienung (FB 2)!", "")),
            "250314_Rechnung_Schröder_Fernbedienung-FB-2",
            "umlauts stay, special characters and spaces do not"
        );
    }

    #[test]
    fn long_names_shorten_subject_first() {
        let f = TemplateFields {
            date: d(2026, 9, 15),
            doctype: "Brief",
            sender: "Finanzamt",
            subject: "Widerruf der Kleinunternehmerregelung für die Photovoltaikanlage auf dem Hausdach",
            person: "Max",
            extra: "",
        };
        let n = name(f);
        assert!(n.chars().count() <= 80, "{n}");
        assert!(n.starts_with("260915_Brief_Finanzamt_Widerruf") && n.ends_with("_Max"), "{n}");
    }

    #[test]
    fn free_style_keeps_spaces() {
        let f = TemplateFields {
            date: d(2024, 3, 12),
            sender: "Stadtwerke Musterstadt",
            doctype: "Rechnung",
            ..Default::default()
        };
        let rules = NameRules { compact: false, max_len: 150 };
        assert_eq!(
            render_filename("{date}_{sender}_{doctype}", "%Y-%m-%d", &f, rules),
            "2024-03-12_Stadtwerke Musterstadt_Rechnung"
        );
        assert_eq!(render_filename("con", "%Y", &TemplateFields::default(), rules), "con_");
    }

    #[test]
    fn persons_are_normalized() {
        let p = persons();
        assert_eq!(normalize_person("Maximilian Muster", &p), "Max");
        assert_eq!(normalize_person("MM", &p), "Max");
        assert_eq!(normalize_person("Lena Muster", &p), "Lena");
        assert_eq!(normalize_person("Paul und Lena", &p), "Paul-Lena");
        assert_eq!(normalize_person("Erika, Max", &p), "Max-Erika", "order as in the list");
        assert_eq!(normalize_person("Max, Erika, Paul, Lena", &p), "Familie");
        assert_eq!(person_in_folder("Max-Erika", "05_Beruf/Gehalt/2026_Max", &p), "Max");
        assert_eq!(person_in_folder("Max", "Steuern/Haus_Maximilian/2025", &p), "Max", "alias");
        assert_eq!(person_in_folder("Max-Erika", "04_Finanzen/Bank/2026", &p), "");
        assert_eq!(person_in_folder("Lena", "07_Familie/Lena", &p), "Lena");
        assert_eq!(person_in_folder("", "07_Familie/Lena", &p), "");
        assert_eq!(normalize_person("familie", &p), "Familie");
        assert_eq!(normalize_person("Elena Petrova", &p), "Elena-Petrova");
        assert_eq!(normalize_person("", &p), "");
    }

    #[test]
    fn senders_are_shortened() {
        let s = vec![
            Alias { name: "MuV".into(), aliases: vec!["M+V".into()] },
            Alias { name: "TK".into(), aliases: vec!["Muster Krankenkasse".into()] },
            Alias { name: "HKM".into(), aliases: vec!["HKM-MUSTERSTADT".into()] },
        ];
        assert_eq!(normalize_sender("M+V Allgemeine Versicherung AG", &s), "MuV");
        assert_eq!(normalize_sender("Muster Krankenkasse", &s), "TK");
        assert_eq!(normalize_sender("HKM-MUSTERSTADT Haftpflicht-Unterstützungs-Kasse", &s), "HKM");
        assert_eq!(normalize_sender("MM Musterholz Möbelwerke GmbH & Co. KG", &s), "MM Musterholz Möbelwerke");
        assert_eq!(normalize_sender("Exempla Versicherungs-AG", &s), "Exempla Versicherung");
        assert_eq!(normalize_sender("Stadtwerke GmbH", &[]), "Stadtwerke");
        assert_eq!(normalize_sender("LEICHTATHLETIK", &[]), "Leichtathletik");
        assert_eq!(normalize_sender("Autohaus-Muster", &[]), "Autohaus-Muster");
        assert_eq!(normalize_sender("EnMS", &[]), "EnMS");
        assert_eq!(normalize_sender("Stadtwerke", &s), "Stadtwerke", "TK is not in it as a word");
    }

    #[test]
    fn doctypes_follow_the_list() {
        let list: Vec<String> = [
            "Rechnung",
            "Steuerbescheid",
            "Kündigungsbestätigung",
            "Kündigung",
            "Gehaltsabrechnung",
            "Schreiben",
            "Urkunde",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(canonical_doctype("rechnung", &list), "Rechnung");
        assert_eq!(canonical_doctype("Rechnungen", &list), "Rechnung");
        assert_eq!(canonical_doctype("Steuerfestsetzung", &list), "Steuerbescheid");
        assert_eq!(canonical_doctype("Brutto-Netto-Abrechnung", &list), "Gehaltsabrechnung");
        assert_eq!(canonical_doctype("Wohngebäude-Kündigungsbestätigung", &list), "Kündigungsbestätigung");
        assert_eq!(canonical_doctype("Leichtathletikurkunde", &list), "Urkunde");
        assert_eq!(canonical_doctype("Typenliste", &list), "Schreiben");
        assert_eq!(canonical_doctype("Typenliste", &[]), "Typenliste");
    }

    #[test]
    fn doctype_refinements() {
        let list: Vec<String> = [
            "Rechnung",
            "Beitragsrechnung",
            "Abrechnung",
            "Gehaltsabrechnung",
            "Bescheid",
            "Steuerbescheid",
            "Grundsteuerbescheid",
            "Kündigung",
            "Kündigungsbestätigung",
            "Bestätigung",
        ]
        .map(String::from)
        .to_vec();
        let r = |s: &str, g: &str| refines(s, g, &list);
        assert!(r("Beitragsrechnung", "Rechnung") && r("Gehaltsabrechnung", "Abrechnung"));
        assert!(r("Grundsteuerbescheid", "Bescheid") && r("Grundsteuerbescheid", "steuerbescheid"));
        assert!(r("Kündigungsbestätigung", "Bestätigung") && !r("Kündigungsbestätigung", "Kündigung"));
        assert!(!r("Gehaltsabrechnung", "Rechnung") && !r("Abrechnung", "Rechnung"), "Ab-rechnung");
        assert!(!r("Bescheid", "Bescheid") && !r("Steuerbescheid", "Bescheinigung") && !r("Bescheid", ""));
        assert_eq!(
            doctype_families(&list),
            vec![
                ("Rechnung", vec!["Beitragsrechnung"]),
                ("Abrechnung", vec!["Gehaltsabrechnung"]),
                ("Bescheid", vec!["Steuerbescheid", "Grundsteuerbescheid"]),
                ("Bestätigung", vec!["Kündigungsbestätigung"]),
            ]
        );
    }

    #[test]
    fn reference_year_prefers_period() {
        assert_eq!(reference_year("2025", d(2026, 9, 25)), Some(2025));
        assert_eq!(reference_year("2025-12", d(2026, 1, 2)), Some(2025));
        assert_eq!(reference_year("2026-Q1", None), Some(2026));
        assert_eq!(reference_year("320d-Bremsen", d(2026, 6, 1)), Some(2026));
        assert_eq!(reference_year("20250-Liter", d(2026, 6, 1)), Some(2026));
        assert_eq!(reference_year("", None), None);
    }

    #[test]
    fn periods_become_year_month() {
        assert_eq!(normalize_period("September-2026"), "2026-09");
        assert_eq!(normalize_period(" August 2026 "), "2026-08");
        assert_eq!(normalize_period("Sept. 2026"), "2026-09");
        assert_eq!(normalize_period("März 2025"), "2025-03");
        assert_eq!(normalize_period("02/2025"), "2025-02");
        assert_eq!(normalize_period("01072026-31072026"), "2026-07");
        assert_eq!(normalize_period("01.02.2024 – 29.02.2024"), "2024-02");
        assert_eq!(normalize_period("2026-09"), "2026-09");
        assert_eq!(normalize_period("01.07.2026-15.07.2026"), "01.07.2026-15.07.2026", "not a whole month");
        assert_eq!(normalize_period("01082026-01082027"), "01082026-01082027");
        assert_eq!(normalize_period("Mai-Ausflug 2026"), "Mai-Ausflug 2026");
        assert_eq!(normalize_period("BMW 320d"), "BMW 320d");
        assert_eq!(normalize_period("2025"), "2025");
    }

    #[test]
    fn program_files_keep_their_name() {
        let p: Vec<String> = ["pfx", ".knxproj", "est*", "s07"].map(String::from).to_vec();
        assert!(keeps_name("PFX", &p) && keeps_name("ESt2024", &p) && keeps_name("knxproj", &p));
        assert!(!keeps_name("pdf", &p) && !keeps_name("es", &p));
    }
}
