//! Archive structure: read folders, apply rules, find year folders, file documents.

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

use crate::config::Rule;
use crate::i18n::files;
use crate::naming::DocDate;

/// Existing folders of the archive as relative paths with `/`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FolderIndex {
    /// All folders down to the configured depth (without excluded ones). For selection and checks.
    pub all: Vec<String>,
    /// Of these, the ones offered to the AI: without the subfolders of collapsed collections.
    pub offered: Vec<String>,
}

impl FolderIndex {
    pub fn scan(root: &Path, depth: usize, exclude: &[String], collapse: &[String]) -> Self {
        let all = scan_folders(root, depth, exclude);
        let collapsed: Vec<String> = collapse.iter().map(|c| normalize(&split_folder(c).join("/"))).collect();
        let offered = all
            .iter()
            .filter(|f| {
                let n = normalize(f);
                !collapsed.iter().any(|c| {
                    !c.is_empty() && n.starts_with(c.as_str()) && n.len() > c.len() && n[c.len()..].starts_with('/')
                })
            })
            .cloned()
            .collect();
        Self { all, offered }
    }

    pub fn exists(&self, folder: &str) -> bool {
        find_existing_folder(&self.all, folder).is_some()
    }
}

/// Folder names that programs create and that are never filing targets.
fn is_hidden_dir(name: &str) -> bool {
    name.starts_with(['.', '@', '#', '~', '$']) || name.eq_ignore_ascii_case("System Volume Information")
}

/// Relative folder paths (with `/`) below the archive, sorted. Hidden and excluded folders (including their subfolders)
/// are skipped.
pub fn scan_folders(root: &Path, depth: usize, exclude: &[String]) -> Vec<String> {
    if !root.is_dir() {
        return Vec::new();
    }
    let excluded: Vec<String> =
        exclude.iter().map(|e| normalize(&split_folder(e).join("/"))).filter(|e| !e.is_empty()).collect();
    let rel_of = |p: &Path| -> Option<String> {
        let rel = p.strip_prefix(root).ok()?;
        let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        Some(parts.join("/"))
    };
    let mut out: Vec<String> = WalkDir::new(root)
        .min_depth(1)
        .max_depth(depth.max(1))
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if is_hidden_dir(&e.file_name().to_string_lossy()) {
                return false;
            }
            let rel = rel_of(e.path()).map(|r| normalize(&r)).unwrap_or_default();
            !excluded
                .iter()
                .any(|x| rel == *x || rel.strip_prefix(x.as_str()).is_some_and(|rest| rest.starts_with('/')))
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
        .filter_map(|e| rel_of(e.path()))
        .collect();
    out.sort_by_key(|s| s.to_lowercase());
    out
}

/// Replaces {year}/{month}/{day} in folder templates. `{year}` is the reference year (period in the subject, otherwise
/// the document date). If a value is missing, the segment is dropped.
pub fn render_folder(template: &str, date: Option<DocDate>, year: Option<i32>) -> String {
    let month = date.and_then(|d| d.month);
    let day = date.and_then(|d| d.day);
    let parts: Vec<String> = split_folder(template)
        .into_iter()
        .filter_map(|seg| {
            let missing = (seg.contains("{year}") && year.is_none())
                || (seg.contains("{month}") && month.is_none())
                || (seg.contains("{day}") && day.is_none());
            if missing {
                return None;
            }
            let mut s = seg.to_string();
            if let Some(y) = year {
                s = s.replace("{year}", &format!("{y:04}"));
            }
            if let Some(m) = month {
                s = s.replace("{month}", &format!("{m:02}"));
            }
            if let Some(d) = day {
                s = s.replace("{day}", &format!("{d:02}"));
            }
            Some(s)
        })
        .collect();
    parts.join("/")
}

/// Removes characters that are not allowed in file and folder names on Windows/macOS/Linux. Spaces and dots in the name
/// stay (existing folders such as "finanzen.net zero").
pub fn sanitize_component(s: &str) -> String {
    let replaced: String =
        s.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { ' ' } else { c }).collect();
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string()
}

pub fn split_folder(s: &str) -> Vec<&str> {
    s.split(['/', '\\']).map(str::trim).filter(|p| !p.is_empty()).collect()
}

/// Turns a folder specification into a safe relative path (no `..`, no drives).
pub fn safe_relative_folder(folder: &str) -> Result<PathBuf> {
    let t = files::t();
    let mut out = PathBuf::new();
    for seg in split_folder(folder) {
        if seg == "." {
            continue;
        }
        if seg == ".." {
            bail!(t.folder_dots);
        }
        let clean = sanitize_component(seg);
        if clean.is_empty() {
            continue;
        }
        out.push(clean);
    }
    if out.components().any(|c| !matches!(c, Component::Normal(_))) {
        bail!((t.folder_invalid)(folder));
    }
    Ok(out)
}

/// Normalizes for comparisons: lower case, umlauts to the base vowel (so "Kontoauszüge" matches "Kontoauszug" and OCR
/// text without umlauts).
pub fn normalize(s: &str) -> String {
    s.to_lowercase().replace('ä', "a").replace('ö', "o").replace('ü', "u").replace('ß', "ss")
}

/// Looks for an existing folder, regardless of case and separators.
pub fn find_existing_folder<'a>(folders: &'a [String], candidate: &str) -> Option<&'a String> {
    let wanted = normalize(&split_folder(candidate).join("/"));
    folders.iter().find(|f| normalize(f) == wanted)
}

/// Matched keywords of a rule; empty if the rule does not apply (disabled, no hits, with `match_all` not all keywords,
/// or an exclusion word).
pub fn rule_hits<'r>(rule: &'r Rule, normalized_text: &str) -> Vec<&'r str> {
    if !rule.enabled {
        return Vec::new();
    }
    let keywords: Vec<&str> = rule.keywords.iter().map(|k| k.trim()).filter(|k| !k.is_empty()).collect();
    let hits: Vec<&str> = keywords.iter().copied().filter(|k| normalized_text.contains(&normalize(k))).collect();
    if rule.match_all && hits.len() < keywords.len() {
        return Vec::new();
    }
    let excluded = rule
        .exclude_keywords
        .iter()
        .map(|k| k.trim())
        .any(|k| !k.is_empty() && normalized_text.contains(&normalize(k)));
    let missing = rule
        .require_keywords
        .iter()
        .map(|k| k.trim())
        .any(|k| !k.is_empty() && !normalized_text.contains(&normalize(k)));
    if excluded || missing {
        return Vec::new();
    }
    hits
}

/// Keywords of a folder, e.g. "Haftpflicht, Grundsteuer & Musterstadt, -Kontoauszug": a comma means "or", " & " (with
/// spaces) means "and", a leading "-" makes it a negative word.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct FolderWords {
    /// Per entry, words that must all occur.
    pub plus: Vec<Vec<String>>,
    pub minus: Vec<Vec<String>>,
}

impl FolderWords {
    pub fn parse(s: &str) -> Self {
        let mut words = Self::default();
        for item in s.split([',', ';', '\n']).map(str::trim).filter(|i| !i.is_empty()) {
            let (negative, item) = match item.strip_prefix('-') {
                Some(rest) => (true, rest.trim()),
                None => (false, item.trim_start_matches('+').trim()),
            };
            let group: Vec<String> =
                item.split(" & ").map(str::trim).filter(|w| !w.is_empty()).map(String::from).collect();
            if group.is_empty() {
                continue;
            }
            if negative { words.minus.push(group) } else { words.plus.push(group) }
        }
        words
    }

    pub fn is_empty(&self) -> bool {
        self.plus.is_empty() && self.minus.is_empty()
    }

    /// Hits in the normalized text: the matching entries (as text) and whether a negative word occurs.
    pub fn hits(&self, normalized_text: &str) -> (Vec<String>, bool) {
        let found = |group: &Vec<String>| group.iter().all(|w| word_in(normalized_text, w));
        let plus = self.plus.iter().filter(|g| found(g)).map(|g| g.join(" & ")).collect();
        (plus, self.minus.iter().any(found))
    }
}

/// Does `word` occur as a whole word in the normalized text (ignoring case and umlauts)? `Versicherung` matches
/// "Kfz-Versicherung", not "Sozialversicherung"; a `*` at the end allows longer words (`Versicherungs*`).
pub fn word_in(normalized_text: &str, word: &str) -> bool {
    let word = word.trim();
    let (word, open_end) = match word.strip_suffix('*') {
        Some(w) => (w, true),
        None => (word, false),
    };
    let w = normalize(word);
    if w.is_empty() {
        return false;
    }
    let edge = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
    normalized_text.match_indices(w.as_str()).any(|(i, _)| {
        edge(normalized_text[..i].chars().next_back())
            && (open_end || edge(normalized_text[i + w.len()..].chars().next()))
    })
}

/// First matching rule. The order of the rules is their priority (specific ones first).
pub fn match_rule_index(rules: &[Rule], text: &str) -> Option<usize> {
    let hay = normalize(text);
    rules.iter().position(|rule| !rule_hits(rule, &hay).is_empty())
}

pub fn match_rule<'a>(rules: &'a [Rule], text: &str) -> Option<&'a Rule> {
    match_rule_index(rules, text).map(|i| &rules[i])
}

/// Existing folder whose names best match the hints (sender, type …).
pub fn best_folder_match(folders: &[String], hints: &[&str]) -> Option<String> {
    let tokens: Vec<String> = hints
        .iter()
        .flat_map(|h| h.split(|c: char| !c.is_alphanumeric()))
        .map(normalize)
        .filter(|t| t.chars().count() >= 4)
        .collect();
    if tokens.is_empty() {
        return None;
    }
    let mut best: Option<(&String, usize, usize)> = None;
    for folder in folders {
        let parts: Vec<String> = folder.split('/').map(normalize).collect();
        let last = parts.last().cloned().unwrap_or_default();
        let mut score = 0;
        for t in &tokens {
            let stem = t.trim_end_matches(['e', 'n', 's']);
            let stem = if stem.chars().count() >= 4 { stem } else { t.as_str() };
            if last.contains(stem) {
                score += 3;
            } else if parts.iter().any(|p| p.contains(stem)) {
                score += 1;
            }
        }
        let depth = parts.len();
        if score > 0 && best.is_none_or(|(_, s, d)| score > s || (score == s && depth > d)) {
            best = Some((folder, score, depth));
        }
    }
    best.map(|(f, _, _)| f.clone())
}

/// Year as a separate part of a folder name: `2026`, `HB_2026`, `2026_Max`, `VR_2026`. Returns (start, end, year) in
/// bytes; other numbers in the name do not interfere (`Konto_2026_123456789`).
pub fn year_span(seg: &str) -> Option<(usize, usize, i32)> {
    let b = seg.as_bytes();
    (0..b.len().saturating_sub(3)).find_map(|i| {
        let digits = b[i..i + 4].iter().all(u8::is_ascii_digit);
        let century = matches!(&b[i..i + 2], b"19" | b"20");
        let sep = |c: u8| matches!(c, b'_' | b'-' | b' ');
        let bounded = (i == 0 || sep(b[i - 1])) && (i + 4 == b.len() || sep(b[i + 4]));
        (digits && century && bounded).then(|| (i, i + 4, seg[i..i + 4].parse().unwrap_or(0)))
    })
}

fn parent_and_last(folder: &str) -> (String, String) {
    let parts = split_folder(folder);
    match parts.split_last() {
        Some((last, parent)) => (parent.join("/"), last.to_string()),
        None => (String::new(), String::new()),
    }
}

fn join(parent: &str, seg: &str) -> String {
    if parent.is_empty() { seg.to_string() } else { format!("{parent}/{seg}") }
}

/// Years of the sibling folders with the same pattern (`HB_2024`, `HB_2025` … → 2024, 2025).
fn series_years(all: &[String], parent: &str, prefix: &str, suffix: &str) -> Vec<i32> {
    let parent_n = normalize(parent);
    let mut years: Vec<i32> = all
        .iter()
        .filter_map(|f| {
            let (p, seg) = parent_and_last(f);
            if normalize(&p) != parent_n {
                return None;
            }
            let (a, b, y) = year_span(&seg)?;
            (normalize(&seg[..a]) == normalize(prefix) && normalize(&seg[b..]) == normalize(suffix)).then_some(y)
        })
        .collect();
    years.sort_unstable();
    years.dedup();
    years
}

/// May the next year folder be created for a running year series? Only without gaps, only the following year and only
/// if the series reaches the previous year.
fn may_extend_series(years: &[i32], target: i32, this_year: i32) -> bool {
    let (Some(&min), Some(&max)) = (years.first(), years.last()) else { return false };
    let contiguous = (max - min + 1) as usize == years.len();
    contiguous && target == max + 1 && max >= this_year - 1
}

/// Folder of an older year in a decade bundle (`Gehalt/202x/2022_Max`, `Depot/201x_Depot/2013`).
fn find_in_bundle(root: &Path, parent: &str, seg: &str, year: i32) -> Option<String> {
    let decade = format!("{}x", year / 10);
    let dir = root.join(safe_relative_folder(parent).ok()?);
    let mut bundles: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| normalize(n).starts_with(&decade))
        .collect();
    bundles.sort();
    bundles.into_iter().find(|b| dir.join(b).join(seg).is_dir()).map(|b| join(parent, &format!("{b}/{seg}")))
}

/// Adapts a year folder to the reference year: `HB_2025` → `HB_2026`, `2025_Erika` → `2026_Erika`. A folder with year
/// subfolders (`_Strom`) leads into the matching year. New year folders are only created as the continuation of a
/// running series; older years are also looked for in decade bundles. `None` means: the folder stays as it is.
pub fn adjust_year_folder(root: &Path, folder: &str, year: i32, all: &[String], this_year: i32) -> Option<String> {
    let (parent, seg) = parent_and_last(folder);
    if let Some((a, b, current)) = year_span(&seg) {
        if current == year {
            return None;
        }
        let (prefix, suffix) = (&seg[..a], &seg[b..]);
        let candidate_seg = format!("{prefix}{year:04}{suffix}");
        let candidate = join(&parent, &candidate_seg);
        if let Some(existing) = find_existing_folder(all, &candidate) {
            return Some(existing.clone());
        }
        if may_extend_series(&series_years(all, &parent, prefix, suffix), year, this_year) {
            return Some(candidate);
        }
        return find_in_bundle(root, &parent, &candidate_seg, year);
    }

    // Folder with year subfolders of a single pattern, e.g. `02_Haus/_Strom` → `_Strom/2026`.
    let folder_n = normalize(&split_folder(folder).join("/"));
    let children: Vec<(String, String, String)> = all
        .iter()
        .filter_map(|f| {
            let (p, s) = parent_and_last(f);
            if normalize(&p) != folder_n {
                return None;
            }
            let (a, b, _) = year_span(&s)?;
            Some((s[..a].to_string(), s[b..].to_string(), f.clone()))
        })
        .collect();
    let (prefix, suffix, _) = children.first()?;
    if children.iter().any(|(p, s, _)| normalize(p) != normalize(prefix) || normalize(s) != normalize(suffix)) {
        return None;
    }
    let candidate_seg = format!("{prefix}{year:04}{suffix}");
    let candidate = join(folder, &candidate_seg);
    if let Some(existing) = find_existing_folder(all, &candidate) {
        return Some(existing.clone());
    }
    if may_extend_series(&series_years(all, folder, prefix, suffix), year, this_year) {
        return Some(candidate);
    }
    find_in_bundle(root, folder, &candidate_seg, year)
}

/// If a folder is missing (e.g. from a rule `Gehalt/{year}_Max` for 2022), it is looked for in the decade bundle.
/// Otherwise the path stays and the folder is created when filing.
pub fn locate_folder(root: &Path, folder: &str, all: &[String]) -> String {
    if let Some(existing) = find_existing_folder(all, folder) {
        return existing.clone();
    }
    let (parent, seg) = parent_and_last(folder);
    if root.join(safe_relative_folder(folder).unwrap_or_default()).is_dir() {
        return folder.to_string();
    }
    match year_span(&seg) {
        Some((_, _, year)) => find_in_bundle(root, &parent, &seg, year).unwrap_or_else(|| folder.to_string()),
        None => folder.to_string(),
    }
}

/// Missing folder whose last part is a year: it is created as a year folder.
pub fn is_year_folder(folder: &str) -> bool {
    split_folder(folder).last().is_some_and(|s| year_span(s).is_some())
}

/// Decade bundle such as `202x` or `201x_Depot`.
pub fn is_decade_bundle(seg: &str) -> bool {
    let b = seg.as_bytes();
    b.len() >= 4
        && matches!(&b[..2], b"19" | b"20")
        && b[2].is_ascii_digit()
        && b[3].eq_ignore_ascii_case(&b'x')
        && (b.len() == 4 || matches!(b[4], b'_' | b'-' | b' '))
}

/// Returns a free target path, if need be with a version (`name_v2.pdf`, `name_v3.pdf`).
pub fn unique_target(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let with_ext = |s: &str| if ext.is_empty() { s.to_string() } else { format!("{s}.{ext}") };
    let first = dir.join(with_ext(stem));
    if !first.exists() {
        return first;
    }
    let mut n = 2u32;
    loop {
        let candidate = dir.join(with_ext(&format!("{stem}_v{n}")));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

pub struct FileRequest<'a> {
    pub source: &'a Path,
    pub archive_root: &'a Path,
    pub folder: &'a str,
    pub filename_stem: &'a str,
    pub copy: bool,
}

/// Result of a filing; enough to undo it.
#[derive(Debug, Clone, PartialEq)]
pub struct Filed {
    pub target: PathBuf,
    pub source: PathBuf,
    pub copied: bool,
    /// Folders newly created while filing, deepest first.
    pub created_dirs: Vec<PathBuf>,
}

/// Moves (or copies) the file to its target. The extension is written in lower case.
pub fn file_document(req: &FileRequest) -> Result<Filed> {
    let t = files::t();
    if !req.source.is_file() {
        bail!((t.source_missing)(&req.source.display().to_string()));
    }
    if req.archive_root.as_os_str().is_empty() {
        bail!(t.no_archive_folder);
    }
    if !req.archive_root.is_dir() {
        bail!((t.archive_folder_missing)(&req.archive_root.display().to_string()));
    }
    let rel = safe_relative_folder(req.folder)?;
    let dir = req.archive_root.join(rel);
    let mut created_dirs = Vec::new();
    let mut probe = dir.as_path();
    while !probe.exists() {
        created_dirs.push(probe.to_path_buf());
        match probe.parent() {
            Some(p) => probe = p,
            None => break,
        }
    }
    std::fs::create_dir_all(&dir).with_context(|| (t.folder_not_created)(&dir.display().to_string()))?;

    let stem = sanitize_component(req.filename_stem);
    let stem = if stem.is_empty() { "Dokument".to_string() } else { stem };
    let ext = req.source.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let target = unique_target(&dir, &stem, &ext);

    let result = if req.copy {
        std::fs::copy(req.source, &target).map(|_| ()).with_context(|| (t.copy_failed)(&target.display().to_string()))
    } else {
        move_file(req.source, &target)
    };
    if let Err(e) = result {
        // Leave nothing half filed: remove newly created, empty folders again.
        remove_empty_dirs(&created_dirs);
        return Err(e);
    }
    Ok(Filed { target, source: req.source.to_path_buf(), copied: req.copy, created_dirs })
}

/// Renames. Only across drives is the file copied and the original deleted afterwards; if it cannot be deleted (e.g.
/// still open in a program), the copy is taken back – otherwise the file would exist twice.
fn move_file(from: &Path, to: &Path) -> Result<()> {
    let t = files::t();
    let err = match std::fs::rename(from, to) {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };
    if !crosses_devices(&err) {
        let message = (t.cannot_move)(&from.display().to_string(), locked(&err));
        return Err(anyhow::Error::new(err).context(message));
    }
    std::fs::copy(from, to).with_context(|| (t.copy_failed)(&to.display().to_string()))?;
    if let Err(e) = std::fs::remove_file(from) {
        std::fs::remove_file(to).ok();
        return Err(anyhow::Error::new(e).context((t.cannot_move_nothing_filed)(&from.display().to_string())));
    }
    Ok(())
}

/// Is the file locked (Windows: ERROR_SHARING_VIOLATION 32, ERROR_LOCK_VIOLATION 33) or is permission missing?
fn locked(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::PermissionDenied || matches!(e.raw_os_error(), Some(32 | 33)) && cfg!(windows)
}

/// Are source and target on different drives?
fn crosses_devices(e: &std::io::Error) -> bool {
    // Windows: ERROR_NOT_SAME_DEVICE (17), Unix: EXDEV (18).
    let code = if cfg!(windows) { 17 } else { 18 };
    e.kind() == std::io::ErrorKind::CrossesDevices || e.raw_os_error() == Some(code)
}

/// Removes folders as long as they are empty (innermost first); otherwise `remove_dir` fails, and that is right.
fn remove_empty_dirs(dirs: &[PathBuf]) {
    for dir in dirs {
        std::fs::remove_dir(dir).ok();
    }
}

/// Undoes a filing: moves the file back to its old place (or deletes the copy) and removes newly created, empty folders
/// again.
pub fn undo_filing(f: &Filed) -> Result<()> {
    let t = files::t();
    if !f.target.is_file() {
        bail!((t.target_missing)(&f.target.display().to_string()));
    }
    if f.copied {
        std::fs::remove_file(&f.target).with_context(|| (t.not_deletable)(&f.target.display().to_string()))?;
    } else {
        if f.source.exists() {
            bail!((t.source_taken)(&f.source.display().to_string()));
        }
        if let Some(dir) = f.source.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        move_file(&f.target, &f.source)?;
    }
    remove_empty_dirs(&f.created_dirs);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_filing_leaves_no_new_folders() {
        let root = tmpdir("fail");
        let missing = root.join("gibt-es-nicht.pdf");
        std::fs::write(&missing, b"x").unwrap();
        let req = FileRequest {
            source: &missing,
            archive_root: &root,
            folder: "Neu/Tief",
            filename_stem: "Name",
            copy: false,
        };
        // The source disappears between the check and the move: the rename fails.
        let created = vec![root.join("Neu/Tief"), root.join("Neu")];
        std::fs::create_dir_all(root.join("Neu/Tief")).unwrap();
        remove_empty_dirs(&created);
        assert!(!root.join("Neu").exists());
        std::fs::remove_file(&missing).unwrap();
        assert!(file_document(&req).is_err(), "source missing");
        assert!(!root.join("Neu").exists());
        let err = move_file(&root.join("fehlt.pdf"), &root.join("ziel.pdf")).unwrap_err();
        assert!(format!("{err:#}").contains("lässt sich nicht verschieben"));
        assert!(!crosses_devices(&std::io::Error::from(std::io::ErrorKind::NotFound)));
        assert!(locked(&std::io::Error::from(std::io::ErrorKind::PermissionDenied)));
    }

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("scrinium-archive-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn full(y: i32, m: u32, d: u32) -> Option<DocDate> {
        chrono::NaiveDate::from_ymd_opt(y, m, d).map(DocDate::full)
    }

    #[test]
    fn folder_words_match_whole_words() {
        let text = normalize("Kfz-Versicherung 2026 · Grundsteuer der Stadt Musterstadt · Kunde bei 1&1, M+V-Police");
        assert!(word_in(&text, "Versicherung") && word_in(&text, "versicherung"));
        assert!(!word_in(&normalize("Meldebescheinigung zur Sozialversicherung"), "Versicherung"));
        assert!(word_in(&normalize("Versicherungsschein Nr. 4"), "Versicherungs*"));
        assert!(word_in(&text, "1&1") && word_in(&text, "M+V") && word_in(&text, "Grundsteuer"));
        assert!(!word_in(&text, "Hundesteuer") && !word_in(&text, ""));

        let w = FolderWords::parse("Grundsteuer & Musterstadt, Hundesteuer, -Kontoauszug, 1&1");
        assert_eq!(w.plus, [vec!["Grundsteuer", "Musterstadt"], vec!["Hundesteuer"], vec!["1&1"]]);
        assert_eq!(w.minus, [vec!["Kontoauszug"]]);
        assert_eq!(w.hits(&text), (vec!["Grundsteuer & Musterstadt".to_string(), "1&1".to_string()], false));
        let statement = normalize("Kontoauszug: Grundsteuer Stadt Musterstadt");
        assert!(w.hits(&statement).1, "negative word");
        assert!(w.hits(&normalize("Grundsteuer Ulm")).0.is_empty(), "\"&\" requires both words");
        assert!(FolderWords::parse(" , ").is_empty());
    }

    #[test]
    fn folder_template() {
        assert_eq!(render_folder("Steuern/{year}", full(2023, 5, 1), Some(2023)), "Steuern/2023");
        assert_eq!(render_folder("Steuern/{year}", None, None), "Steuern");
        assert_eq!(render_folder("Gehalt/{year}_Max", full(2026, 1, 2), Some(2025)), "Gehalt/2025_Max", "ref. year");
        assert_eq!(render_folder("A/{year}-{month}", DocDate::year_month(2016, 1), Some(2016)), "A/2016-01");
        assert_eq!(render_folder("A/{day}", DocDate::year_month(2016, 1), Some(2016)), "A");
        assert_eq!(render_folder("\\Bank\\Konto/", None, None), "Bank/Konto");
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(safe_relative_folder("../etc").is_err());
        assert_eq!(safe_relative_folder("/Versicherung/Auto").unwrap(), PathBuf::from("Versicherung").join("Auto"));
        assert_eq!(safe_relative_folder("").unwrap(), PathBuf::new());
        assert_eq!(
            safe_relative_folder("04_Finanzen/finanzen.net zero").unwrap(),
            PathBuf::from("04_Finanzen/finanzen.net zero")
        );
    }

    #[test]
    fn rule_order_is_priority() {
        let rules = vec![
            Rule { keywords: vec!["Brutto-Netto-Abrechnung".into()], folder: "Gehalt".into(), ..Default::default() },
            Rule {
                keywords: vec!["Finanzamt".into(), "Steuer".into()],
                folder: "Steuern".into(),
                ..Default::default()
            },
        ];
        assert_eq!(match_rule(&rules, "Brutto-Netto-Abrechnung, Steuer an Finanzamt").unwrap().folder, "Gehalt");
        assert_eq!(match_rule(&rules, "Finanzamt Ulm").unwrap().folder, "Steuern");
        assert!(match_rule(&rules, "Kontoauszug").is_none());
    }

    #[test]
    fn disabled_match_all_and_excluded_rules() {
        let rules = vec![
            Rule { keywords: vec!["Rechnung".into()], folder: "Aus".into(), enabled: false, ..Default::default() },
            Rule {
                keywords: vec!["Exempla".into(), "Kfz".into()],
                folder: "Auto".into(),
                match_all: true,
                ..Default::default()
            },
            Rule {
                keywords: vec!["Grundsteuer".into()],
                exclude_keywords: vec!["Kontoauszug".into()],
                folder: "Steuern/Grundsteuer".into(),
                ..Default::default()
            },
            Rule { keywords: vec!["Exempla".into()], folder: "Versicherungen".into(), ..Default::default() },
        ];
        assert!(match_rule(&rules, "Ihre Rechnung").is_none(), "disabled rule does not apply");
        assert_eq!(match_rule(&rules, "Exempla Hausrat").unwrap().folder, "Versicherungen");
        assert_eq!(match_rule(&rules, "Exempla KFZ-Versicherung").unwrap().folder, "Auto");
        assert_eq!(match_rule(&rules, "Grundsteuerbescheid").unwrap().folder, "Steuern/Grundsteuer");
        assert!(match_rule(&rules, "Kontoauszug: Lastschrift Grundsteuer").is_none(), "exclusion word");
        let local = Rule {
            keywords: vec!["Abfallgebühren".into()],
            require_keywords: vec!["Musterstadt".into()],
            folder: "02_Haus/_Wasser_Abfall".into(),
            ..Default::default()
        };
        assert!(match_rule(std::slice::from_ref(&local), "Abfallgebühren Anderstadt").is_none(), "other town");
        assert!(match_rule(std::slice::from_ref(&local), "Stadt Musterstadt – Abfallgebühren 2026").is_some());
        assert_eq!(rule_hits(&rules[1], &normalize("Exempla Kfz")), vec!["Exempla", "Kfz"]);
    }

    #[test]
    fn folder_matching() {
        let folders =
            strings(&["Bank", "Bank/Kontoauszüge", "Versicherungen", "Versicherungen/Exempla", "Wohnung/Nebenkosten"]);
        assert_eq!(best_folder_match(&folders, &["Exempla Versicherung", "Police"]).unwrap(), "Versicherungen/Exempla");
        assert_eq!(best_folder_match(&folders, &["Sparkasse", "Kontoauszug"]).unwrap(), "Bank/Kontoauszüge");
        assert!(best_folder_match(&folders, &["xyz"]).is_none());
        assert_eq!(find_existing_folder(&folders, "wohnung\\nebenkosten").unwrap(), "Wohnung/Nebenkosten");
    }

    #[test]
    fn scans_with_exclude_and_collapse() {
        let root = tmpdir("scan");
        for d in [
            "Steuern/2023",
            "Bank/Giro/Alt/Tief",
            ".git/objects",
            "@eaDir",
            "_Umstellung_C/Logs",
            "90_Wissen/Rezepte/Kuchen",
        ] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::write(root.join("Bank/datei.pdf"), b"x").unwrap();
        let idx = FolderIndex::scan(&root, 3, &strings(&["_Umstellung_C"]), &strings(&["90_Wissen/Rezepte"]));
        assert_eq!(
            idx.all,
            strings(&[
                "90_Wissen",
                "90_Wissen/Rezepte",
                "90_Wissen/Rezepte/Kuchen",
                "Bank",
                "Bank/Giro",
                "Bank/Giro/Alt",
                "Steuern",
                "Steuern/2023"
            ])
        );
        assert_eq!(
            idx.offered,
            strings(&[
                "90_Wissen",
                "90_Wissen/Rezepte",
                "Bank",
                "Bank/Giro",
                "Bank/Giro/Alt",
                "Steuern",
                "Steuern/2023"
            ])
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn year_folders_follow_the_document() {
        let root = tmpdir("years");
        for d in ["Gehalt/202x/2022_Max", "Depot/201x_Depot/2013"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        let all = strings(&[
            "Gehalt",
            "Gehalt/202x",
            "Gehalt/2024_Erika",
            "Gehalt/2025_Erika",
            "Gehalt/2025_Max",
            "Gehalt/2026_Max",
            "Kreditkarte/KK_2025",
            "Kreditkarte/KK_2026",
            "_Strom",
            "_Strom/2025",
            "_Strom/2026",
            "Serien/Office_2016",
            "Serien/Office_2021",
            "Depot",
            "Depot/201x_Depot",
            "Depot/2025",
            "Depot/2026",
        ]);
        let adj = |f: &str, y: i32| adjust_year_folder(&root, f, y, &all, 2026);
        assert_eq!(adj("Kreditkarte/KK_2025", 2026).as_deref(), Some("Kreditkarte/KK_2026"), "exists");
        assert_eq!(adj("Gehalt/2025_Erika", 2026).as_deref(), Some("Gehalt/2026_Erika"), "new following year");
        assert_eq!(adj("Gehalt/2026_Max", 2022).as_deref(), Some("Gehalt/202x/2022_Max"), "in the bundle");
        assert_eq!(adj("Depot/2026", 2013).as_deref(), Some("Depot/201x_Depot/2013"));
        assert_eq!(adj("Gehalt/2025_Erika", 2019), None, "do not make up a folder");
        assert_eq!(adj("Serien/Office_2021", 2026), None, "version numbers are not a year series");
        assert_eq!(adj("_Strom", 2026).as_deref(), Some("_Strom/2026"), "into the year folder");
        assert_eq!(adj("_Strom", 2027).as_deref(), Some("_Strom/2027"));
        assert_eq!(adj("Gehalt", 2026), None, "several patterns: do not guess");
        assert_eq!(adj("Kreditkarte/KK_2026", 2026), None);
        assert_eq!(locate_folder(&root, "Gehalt/2022_Max", &all), "Gehalt/202x/2022_Max");
        assert_eq!(locate_folder(&root, "Gehalt/2026_Erika", &all), "Gehalt/2026_Erika");
        assert!(is_year_folder("Gehalt/2026_Erika") && !is_year_folder("Gehalt/Kinder"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn files_with_collision_handling_and_undo() {
        let root = tmpdir("file");
        let inbox = root.join("inbox");
        std::fs::create_dir_all(&inbox).unwrap();
        let archive = root.join("archiv");
        std::fs::create_dir_all(&archive).unwrap();
        let mut filed = Vec::new();
        for i in 0..3 {
            let src = inbox.join(format!("scan{i}.PDF"));
            std::fs::write(&src, format!("inhalt {i}")).unwrap();
            let f = file_document(&FileRequest {
                source: &src,
                archive_root: &archive,
                folder: "08_Steuern/2024",
                filename_stem: "240101_Steuerbescheid_Finanzamt_2023",
                copy: i == 2,
            })
            .unwrap();
            let expected = match i {
                0 => "240101_Steuerbescheid_Finanzamt_2023.pdf",
                1 => "240101_Steuerbescheid_Finanzamt_2023_v2.pdf",
                _ => "240101_Steuerbescheid_Finanzamt_2023_v3.pdf",
            };
            assert_eq!(f.target, archive.join("08_Steuern/2024").join(expected));
            assert_eq!(std::fs::read_to_string(&f.target).unwrap(), format!("inhalt {i}"));
            assert_eq!(src.exists(), i == 2, "original kept after a copy, otherwise moved");
            assert_eq!(f.created_dirs.len(), if i == 0 { 2 } else { 0 });
            filed.push(f);
        }
        for f in filed.iter().rev() {
            undo_filing(f).unwrap();
        }
        assert!(
            inbox.join("scan0.PDF").is_file() && inbox.join("scan1.PDF").is_file() && inbox.join("scan2.PDF").is_file()
        );
        assert!(!archive.join("08_Steuern").exists(), "newly created, empty folders are gone again");
        std::fs::remove_dir_all(root).ok();
    }
}
