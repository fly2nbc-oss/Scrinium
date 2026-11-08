//! Properties for Windows Search: title, subject, tags and comment are written into the file when
//! filing – where Explorer reads them under "Details" and the search index finds them
//! (`Markierungen:Steuerbescheid`, `Titel:Finanzamt` in a German Windows).
//!
//! - PDF: Info dictionary and XMP, appended as an incremental update; the previous content stays
//!   intact byte for byte. Signed and encrypted PDFs are left untouched.
//! - Word, Excel, PowerPoint: `docProps/core.xml` (title, subject, tags, comments).
//! - Images (JPG, PNG, TIFF, WebP): on Windows via the property system, just like editing in
//!   Explorer.
//!
//! Existing tags are kept, new ones are added.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{Context, Result};
use regex::Regex;

use crate::i18n::files;

pub(crate) use pdf::is_signed;

/// What is written into the file. Empty fields leave the existing value in place.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tags {
    pub title: String,
    pub subject: String,
    pub keywords: Vec<String>,
    pub comment: String,
}

/// Fields of a confirmed suggestion.
#[derive(Debug, Default)]
pub struct Source<'a> {
    pub doctype: &'a str,
    pub sender: &'a str,
    pub subject: &'a str,
    pub person: &'a str,
    pub extra: &'a str,
    pub year: Option<i32>,
    pub folder: &'a str,
    pub summary: &'a str,
}

const MAX_KEYWORDS: usize = 12;

impl Tags {
    /// Title "Doctype Sender – Subject", tags from type, sender, persons, year, the folder names (without numbers and
    /// years) and the suffix, comment from the summary.
    pub fn build(s: &Source) -> Tags {
        let mut keywords: Vec<String> = Vec::new();
        let mut add = |k: &str| {
            let k = k.trim();
            if !k.is_empty() && keywords.len() < MAX_KEYWORDS && !keywords.iter().any(|e| e.eq_ignore_ascii_case(k)) {
                keywords.push(k.to_string());
            }
        };
        add(s.doctype);
        add(s.sender);
        s.person.split('-').for_each(&mut add);
        if let Some(y) = s.year {
            add(&y.to_string());
        }
        folder_categories(s.folder).iter().for_each(|c| add(c));
        add(s.extra);
        let head = [s.doctype, s.sender].map(str::trim).into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>();
        let subject = s.subject.trim();
        let title = match (head.is_empty(), subject.is_empty()) {
            (false, false) => format!("{} – {subject}", head.join(" ")),
            (false, true) => head.join(" "),
            (true, _) => subject.to_string(),
        };
        Tags { title, subject: subject.to_string(), keywords, comment: s.summary.trim().to_string() }
    }
}

/// Folder names as keywords: `01_Versicherung/MuV_Versicherung` → "Versicherung", "MuV Versicherung". Year folders
/// (`2026`, `HB_2026`, `202x`) do not count.
fn folder_categories(folder: &str) -> Vec<String> {
    static NUMBERED: LazyLock<Regex> = LazyLock::new(|| crate::heuristics::code_regex(r"^\d+_"));
    static YEAR: LazyLock<Regex> =
        LazyLock::new(|| crate::heuristics::code_regex(r"(?:^|[_\- ])(?:19|20)\d[\dx](?:$|[_\- ])"));
    folder
        .split(['/', '\\'])
        .filter(|s| !s.is_empty() && !YEAR.is_match(s))
        .map(|s| NUMBERED.replace(s, "").trim_matches('_').replace('_', " ").trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Result for one file.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Written,
    /// Format without properties that Windows reads (text, Markdown, OpenDocument …).
    Unsupported,
    /// Deliberately not changed, with a reason.
    Skipped(String),
}

/// Can this file type be written to?
pub fn supports(path: &Path) -> bool {
    !matches!(kind(path), Kind::Other)
}

enum Kind {
    Pdf,
    Ooxml,
    Image,
    Other,
}

fn kind(path: &Path) -> Kind {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "pdf" => Kind::Pdf,
        "docx" | "docm" | "dotx" | "xlsx" | "xlsm" | "pptx" | "pptm" => Kind::Ooxml,
        "jpg" | "jpeg" | "png" | "tif" | "tiff" | "webp" if cfg!(windows) => Kind::Image,
        _ => Kind::Other,
    }
}

/// Writes the properties into the file. Errors leave the file unchanged.
pub fn write(path: &Path, tags: &Tags) -> Result<Outcome> {
    match kind(path) {
        Kind::Pdf => pdf::write(path, tags),
        Kind::Ooxml => ooxml::write(path, tags),
        Kind::Image => image::write(path, tags),
        Kind::Other => Ok(Outcome::Unsupported),
    }
}

/// Existing and new tags without duplicates; split at `;` and `,`.
fn merge_keywords(existing: &str, new: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for k in existing.split([';', ',']).map(str::trim).chain(new.iter().map(|k| k.trim())) {
        if !k.is_empty() && !out.iter().any(|e| e.eq_ignore_ascii_case(k)) {
            out.push(k.to_string());
        }
    }
    out
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn unescape_xml(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Removes an element including its content (`<dc:title>…</dc:title>` or `<dc:title/>`).
fn remove_element(xml: &str, name: &str) -> String {
    let n = regex::escape(name);
    let re = crate::heuristics::code_regex(&format!(r"(?s)<{n}\b[^>]*?/>|<{n}\b[^>]*>.*?</{n}\s*>"));
    re.replace_all(xml, "").into_owned()
}

/// Text of a simple element, e.g. `<cp:keywords>a; b</cp:keywords>`.
fn element_text(xml: &str, name: &str) -> String {
    let n = regex::escape(name);
    let re = crate::heuristics::code_regex(&format!(r"(?s)<{n}\b[^>]*>(.*?)</{n}\s*>"));
    re.captures(xml).map(|c| unescape_xml(&c[1])).unwrap_or_default()
}

/// Replaces the file via a temporary file in the same folder. If something fails, the original stays unchanged. If
/// another program (virus scanner, sync, search index) holds the file open briefly, the replace is retried a few times.
pub(crate) fn replace_with(path: &Path, write: impl FnOnce(&mut File) -> Result<()>) -> Result<()> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp: PathBuf = path.with_file_name(format!(".~scrinium-{name}"));
    let t = files::t();
    let written = (|| -> Result<()> {
        let mut f = File::create(&tmp).with_context(|| (t.file_not_created)(&tmp.display().to_string()))?;
        write(&mut f)?;
        f.sync_all()?;
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    let mut last = None;
    for attempt in 0..6 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        match std::fs::rename(&tmp, path) {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
    }
    let _ = std::fs::remove_file(&tmp);
    Err(last.map(anyhow::Error::from).unwrap_or_else(|| anyhow::anyhow!(t.replace_failed)))
        .with_context(|| (t.file_in_use)(&path.display().to_string()))
}

mod pdf {
    use super::*;
    use lopdf::{Dictionary, Document, IncrementalDocument, Object, Stream, decode_text_string, text_string};

    /// Info subject: subject and summary – PDF has no comment field of its own.
    fn subject_line(tags: &Tags) -> String {
        [tags.subject.as_str(), tags.comment.as_str()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" – ")
    }

    pub fn write(path: &Path, tags: &Tags) -> Result<Outcome> {
        let t = files::t();
        let mut doc = IncrementalDocument::load(path).context(t.pdf_not_read)?;
        let prev = doc.get_prev_documents();
        if prev.is_encrypted() {
            return Ok(Outcome::Skipped(t.skip_encrypted.into()));
        }
        if is_signed(prev) {
            return Ok(Outcome::Skipped(t.skip_signed.into()));
        }
        let root_id = prev.trailer.get(b"Root").and_then(Object::as_reference).context(t.pdf_no_catalog)?;
        let mut catalog = prev.get_object(root_id).and_then(Object::as_dict).context(t.pdf_no_catalog)?.clone();

        let info_id = prev.trailer.get(b"Info").and_then(Object::as_reference).ok();
        let mut info = match prev.trailer.get(b"Info") {
            Ok(Object::Reference(id)) => prev.get_object(*id).and_then(Object::as_dict).cloned().unwrap_or_default(),
            Ok(Object::Dictionary(d)) => d.clone(),
            _ => Dictionary::new(),
        };
        let existing = info.get(b"Keywords").ok().and_then(|o| decode_text_string(o).ok()).unwrap_or_default();
        let keywords = merge_keywords(&existing, &tags.keywords);
        let subject = subject_line(tags);
        for (key, value) in
            [(&b"Title"[..], tags.title.clone()), (b"Subject", subject.clone()), (b"Keywords", keywords.join("; "))]
        {
            if !value.is_empty() {
                info.set(key, text_string(&value));
            }
        }

        let xmp_id = catalog.get(b"Metadata").and_then(Object::as_reference).ok();
        let existing_xmp = xmp_id
            .and_then(|id| prev.get_object(id).ok())
            .and_then(|o| o.as_stream().ok())
            .map(|s| s.decompressed_content().unwrap_or_else(|_| s.content.clone()))
            .and_then(|b| String::from_utf8(b).ok());
        let xmp = merge_xmp(existing_xmp.as_deref(), &tags.title, &subject, &keywords);

        let new = &mut doc.new_document;
        let info_id = match info_id {
            Some(id) => {
                new.set_object(id, info);
                id
            }
            None => new.add_object(info),
        };
        let mut dict = Dictionary::new();
        dict.set("Type", Object::Name(b"Metadata".to_vec()));
        dict.set("Subtype", Object::Name(b"XML".to_vec()));
        // Uncompressed, so that programs that only search for the XMP packet find it too.
        let stream = Stream::new(dict, xmp.into_bytes()).with_compression(false);
        let xmp_id = match xmp_id {
            Some(id) => {
                new.set_object(id, stream);
                id
            }
            None => new.add_object(stream),
        };
        catalog.set("Metadata", Object::Reference(xmp_id));
        new.set_object(root_id, catalog);
        new.trailer.set("Info", Object::Reference(info_id));
        replace_with(path, |f| {
            let mut out = std::io::BufWriter::new(f);
            doc.save_to(&mut out).context(t.pdf_not_written)?;
            out.flush()?;
            Ok(())
        })?;
        Ok(Outcome::Written)
    }

    /// Signed PDFs (signature or timestamp dictionaries) are not touched.
    pub(crate) fn is_signed(doc: &Document) -> bool {
        doc.objects.values().any(|o| {
            let d = match o {
                Object::Dictionary(d) => d,
                Object::Stream(s) => &s.dict,
                _ => return false,
            };
            d.has(b"ByteRange")
                || d.get(b"Type").and_then(Object::as_name).is_ok_and(|n| n == b"Sig" || n == b"DocTimeStamp")
        })
    }
}

/// Sets title, description and keywords in the XMP packet. Existing values of these fields are replaced, everything
/// else (producer, PDF/A marker …) stays; the fields go into their own `rdf:Description`.
fn merge_xmp(existing: Option<&str>, title: &str, description: &str, keywords: &[String]) -> String {
    let alt = |name: &str, v: &str| {
        format!(r#"<{name}><rdf:Alt><rdf:li xml:lang="x-default">{}</rdf:li></rdf:Alt></{name}>"#, escape_xml(v))
    };
    let mut ours = String::new();
    let mut replaced: Vec<&str> = Vec::new();
    if !title.is_empty() {
        ours += &alt("dc:title", title);
        replaced.push("dc:title");
    }
    if !description.is_empty() {
        ours += &alt("dc:description", description);
        replaced.push("dc:description");
    }
    if !keywords.is_empty() {
        let items: String = keywords.iter().map(|k| format!("<rdf:li>{}</rdf:li>", escape_xml(k))).collect();
        ours += &format!("<dc:subject><rdf:Bag>{items}</rdf:Bag></dc:subject>");
        ours += &format!("<pdf:Keywords>{}</pdf:Keywords>", escape_xml(&keywords.join("; ")));
        replaced.extend(["dc:subject", "pdf:Keywords"]);
    }
    let ours = format!(
        r#"<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:pdf="http://ns.adobe.com/pdf/1.3/">{ours}</rdf:Description>"#
    );
    match existing.filter(|x| x.contains("</rdf:RDF>")) {
        Some(x) => {
            let mut x = x.to_string();
            for name in &replaced {
                x = remove_element(&x, name);
                let attr =
                    crate::heuristics::code_regex(&format!(r#"\s{}\s*=\s*("[^"]*"|'[^']*')"#, regex::escape(name)));
                x = attr.replace_all(&x, "").into_owned();
            }
            let pos = x.rfind("</rdf:RDF>").unwrap_or(x.len());
            x.insert_str(pos, &ours);
            x
        }
        None => format!(
            "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
             <x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
             {ours}</rdf:RDF></x:xmpmeta>\n<?xpacket end=\"w\"?>"
        ),
    }
}

mod ooxml {
    use super::*;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipArchive, ZipWriter};

    const CORE: &str = "docProps/core.xml";

    pub fn write(path: &Path, tags: &Tags) -> Result<Outcome> {
        let t = files::t();
        let mut zip = ZipArchive::new(File::open(path)?).context(t.office_not_read)?;
        let core = match zip.by_name(CORE) {
            Ok(mut f) => {
                let mut s = String::new();
                f.read_to_string(&mut s)?;
                s
            }
            Err(_) => return Ok(Outcome::Skipped(t.skip_no_properties.into())),
        };
        let Some(core) = merge_core(&core, tags) else {
            return Ok(Outcome::Skipped(t.skip_unknown_properties.into()));
        };
        replace_with(path, |out| {
            let mut w = ZipWriter::new(out);
            for i in 0..zip.len() {
                let entry = zip.by_index_raw(i)?;
                if entry.name().is_ok_and(|n| n == CORE) {
                    drop(entry);
                    w.start_file(CORE, SimpleFileOptions::default().compression_method(CompressionMethod::Deflated))?;
                    w.write_all(core.as_bytes())?;
                } else {
                    w.raw_copy_file(entry)?;
                }
            }
            w.finish()?;
            Ok(())
        })?;
        Ok(Outcome::Written)
    }

    /// Title → `dc:title`, subject → `dc:subject`, tags → `cp:keywords`, comments → `dc:description` – this is how
    /// Explorer reads them.
    pub(super) fn merge_core(xml: &str, tags: &Tags) -> Option<String> {
        if !xml.contains("<cp:coreProperties") || !xml.contains("xmlns:dc=") {
            return None;
        }
        let keywords = merge_keywords(&element_text(xml, "cp:keywords"), &tags.keywords).join("; ");
        let mut x = xml.to_string();
        for (name, value) in [
            ("dc:title", &tags.title),
            ("dc:subject", &tags.subject),
            ("cp:keywords", &keywords),
            ("dc:description", &tags.comment),
        ] {
            if value.is_empty() {
                continue;
            }
            x = remove_element(&x, name);
            let pos = x.rfind("</cp:coreProperties>")?;
            x.insert_str(pos, &format!("<{name}>{}</{name}>", escape_xml(value)));
        }
        Some(x)
    }
}

#[cfg(windows)]
mod image {
    use super::*;
    use windows::Win32::Storage::EnhancedStorage::{PKEY_Comment, PKEY_Keywords, PKEY_Subject, PKEY_Title};
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
    use windows::Win32::UI::Shell::PropertiesSystem::{
        GPS_READWRITE, IPropertyStore, PSCoerceToCanonicalValue, SHGetPropertyStoreFromParsingName,
    };
    use windows::core::{HSTRING, PROPVARIANT};

    /// Via the Windows property system, in a separate thread with COM.
    pub fn write(path: &Path, tags: &Tags) -> Result<Outcome> {
        let (path, tags) = (path.to_path_buf(), tags.clone());
        std::thread::spawn(move || unsafe { write_store(&path, &tags) })
            .join()
            .map_err(|_| anyhow::anyhow!(files::t().properties_thread))??;
        Ok(Outcome::Written)
    }

    unsafe fn write_store(path: &Path, tags: &Tags) -> Result<()> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
        let result = (|| -> Result<()> {
            let store: IPropertyStore =
                unsafe { SHGetPropertyStoreFromParsingName(&HSTRING::from(path), None, GPS_READWRITE) }
                    .context(files::t().properties_not_opened)?;
            let existing = unsafe { store.GetValue(&PKEY_Keywords) }.map(|v| v.to_string()).unwrap_or_default();
            let keywords = merge_keywords(&existing, &tags.keywords).join("; ");
            for (key, value) in [
                (PKEY_Title, &tags.title),
                (PKEY_Subject, &tags.subject),
                (PKEY_Keywords, &keywords),
                (PKEY_Comment, &tags.comment),
            ] {
                if value.is_empty() {
                    continue;
                }
                let mut v = PROPVARIANT::from(value.as_str());
                // "a; b; c" becomes a list for tags and plain text for the title.
                unsafe { PSCoerceToCanonicalValue(&key, &mut v) }?;
                unsafe { store.SetValue(&key, &v) }?;
            }
            unsafe { store.Commit() }.context(files::t().properties_not_saved)?;
            Ok(())
        })();
        unsafe { CoUninitialize() };
        result
    }
}

#[cfg(not(windows))]
mod image {
    use super::*;

    pub fn write(_path: &Path, _tags: &Tags) -> Result<Outcome> {
        Ok(Outcome::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags() -> Tags {
        Tags::build(&Source {
            doctype: "Beitragsrechnung",
            sender: "MuV",
            subject: "Haftpflicht",
            person: "Max-Erika",
            extra: "",
            year: Some(2026),
            folder: "01_Versicherung/MuV_Versicherung/2026",
            summary: "Jahresbeitrag 143,72 € für die Haftpflicht.",
        })
    }

    #[test]
    fn builds_title_and_keywords() {
        let t = tags();
        assert_eq!(t.title, "Beitragsrechnung MuV – Haftpflicht");
        assert_eq!(t.keywords, ["Beitragsrechnung", "MuV", "Max", "Erika", "2026", "Versicherung", "MuV Versicherung"]);
        assert_eq!(t.comment, "Jahresbeitrag 143,72 € für die Haftpflicht.");
        assert_eq!(folder_categories("05_Beruf/Gehalt/202x/2022_Max"), ["Beruf", "Gehalt"]);
        assert_eq!(folder_categories("02_Haus/_Strom/HB_2026"), ["Haus", "Strom"]);
        assert_eq!(merge_keywords("alt; MuV, Foto", &["muv".into(), "Neu".into()]), ["alt", "MuV", "Foto", "Neu"]);
    }

    #[test]
    fn xmp_keeps_other_properties() {
        let old = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?><x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/" pdfaid:part="2" dc:format="application/pdf"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Scan</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end="w"?>"#;
        let x = merge_xmp(Some(old), "Steuerbescheid & Co", "", &["2025".into()]);
        assert!(x.contains(r#"pdfaid:part="2""#) && x.contains("dc:format"));
        assert!(!x.contains(">Scan<") && x.contains("Steuerbescheid &amp; Co"));
        assert_eq!(x.matches("<dc:title>").count(), 1);
        assert!(x.contains("<pdf:Keywords>2025</pdf:Keywords>") && x.ends_with(r#"<?xpacket end="w"?>"#));
        let fresh = merge_xmp(None, "Titel", "Beschreibung", &[]);
        assert!(fresh.contains("<dc:description>") && !fresh.contains("dc:subject"));
    }

    #[test]
    fn office_core_properties() {
        let core = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Alt</dc:title><dc:creator>Max</dc:creator><cp:keywords>Eigen</cp:keywords></cp:coreProperties>"#;
        let x = ooxml::merge_core(core, &tags()).unwrap();
        assert!(x.contains("<dc:title>Beitragsrechnung MuV – Haftpflicht</dc:title>") && !x.contains(">Alt<"));
        assert!(x.contains("<dc:creator>Max</dc:creator>"));
        assert!(x.contains("<cp:keywords>Eigen; Beitragsrechnung; MuV;"));
        assert!(x.contains("<dc:description>Jahresbeitrag") && x.contains("<dc:subject>Haftpflicht</dc:subject>"));
        assert!(ooxml::merge_core("<coreProperties/>", &tags()).is_none());
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("scrinium-tags-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn pdf_gets_info_and_xmp_as_update() {
        use lopdf::{Document, Object, Stream, dictionary};
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content = doc.add_object(Stream::new(dictionary! {}, b"BT ET".to_vec()));
        let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => content });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        let path = dir("pdf").join("Bescheid.pdf");
        doc.save(&path).unwrap();
        let before = std::fs::read(&path).unwrap();

        assert_eq!(write(&path, &tags()).unwrap(), Outcome::Written);
        let after = std::fs::read(&path).unwrap();
        assert!(after.starts_with(&before), "incremental: old content unchanged at the front");
        let doc = Document::load(&path).unwrap();
        let info = doc.trailer.get(b"Info").and_then(Object::as_reference).and_then(|id| doc.get_object(id)).unwrap();
        let title = lopdf::decode_text_string(info.as_dict().unwrap().get(b"Title").unwrap()).unwrap();
        assert_eq!(title, "Beitragsrechnung MuV – Haftpflicht");
        assert_eq!(doc.get_pages().len(), 1);
        let meta = doc.catalog().unwrap().get(b"Metadata").and_then(Object::as_reference).unwrap();
        let xmp = String::from_utf8(doc.get_object(meta).unwrap().as_stream().unwrap().content.clone()).unwrap();
        assert!(xmp.contains("<rdf:li>Beitragsrechnung</rdf:li>"));

        // Second time: tags not duplicated.
        assert_eq!(write(&path, &tags()).unwrap(), Outcome::Written);
        let doc = Document::load(&path).unwrap();
        let info = doc.trailer.get(b"Info").and_then(Object::as_reference).and_then(|id| doc.get_object(id)).unwrap();
        let kw = lopdf::decode_text_string(info.as_dict().unwrap().get(b"Keywords").unwrap()).unwrap();
        assert_eq!(kw.matches("MuV;").count(), 1, "{kw}");
    }

    #[test]
    fn office_file_is_rewritten_entry_by_entry() {
        use zip::write::SimpleFileOptions;
        let path = dir("docx").join("Brief.docx");
        {
            let mut w = zip::ZipWriter::new(File::create(&path).unwrap());
            for (name, data) in [
                ("[Content_Types].xml", "<Types/>"),
                ("docProps/core.xml", r#"<cp:coreProperties xmlns:cp="x" xmlns:dc="y"></cp:coreProperties>"#),
                ("word/document.xml", "<w:document>Inhalt</w:document>"),
            ] {
                w.start_file(name, SimpleFileOptions::default()).unwrap();
                w.write_all(data.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        assert_eq!(write(&path, &tags()).unwrap(), Outcome::Written);
        let mut zip = zip::ZipArchive::new(File::open(&path).unwrap()).unwrap();
        assert_eq!(zip.file_names().count(), 3);
        assert_eq!(zip.by_index(0).unwrap().name().unwrap(), "[Content_Types].xml");
        let mut core = String::new();
        zip.by_name("docProps/core.xml").unwrap().read_to_string(&mut core).unwrap();
        assert!(core.contains("<cp:keywords>Beitragsrechnung; MuV;"));
        let mut body = String::new();
        zip.by_name("word/document.xml").unwrap().read_to_string(&mut body).unwrap();
        assert_eq!(body, "<w:document>Inhalt</w:document>");
        assert_eq!(write(&dir("txt").join("notiz.md"), &tags()).unwrap(), Outcome::Unsupported);
    }
}
