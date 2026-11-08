# File names

Scrinium builds the name from **parts**. Empty parts are dropped along with their separator. So `{date}_{doctype}_{sender}_{subject}` becomes `260925_Steuerbescheid_Finanzamt_2025.pdf` or, without a date, `Mitteilung_BZSt_Steuer-ID.pdf`.

## Set up the structure

Under *Settings › File names › Structure*:

- **Drag parts** to change the order. The cross removes a part; **+ Part** adds a missing one or a custom text (e.g. `Scan`).
- **Separator** between the parts: underscore, hyphen or space.
- **Date** as `YYMMDD`, `YYYY-MM-DD` or `YYYYMMDD`; any other `strftime` format under *Advanced › Date format*.
- **As text** shows the template as it is stored. Here you can write anything the chips cannot show, such as `{year}-{month}` in one part. *Templates* next to it sets one of the usual orders.
- **Names will look like this** shows three examples with the current settings.

In the main window you can click any part to change it for a single document, or leave it out with the cross; *Original name* takes the current name.

## Parts

| Part | Rule | Example |
|---|---|---|
| `{date}` | Document date; an unknown day or month becomes `00`. With several dates, a letter date (“Ort, den …”, “Datum: …”) comes before “vom …”/“am …”, which come before dates without a hint; deadlines and periods come last. On the same level, the date closest to today wins | `260925`, `160100` |
| `{doctype}` | from the list *Document types* (singular), the most precise matching term | `Steuerbescheid`, `Kontoauszug` |
| `{sender}` | short name from *Short names for senders*, otherwise without the legal form (GmbH, AG, e.V. …) | `SWM`, `Finanzamt` |
| `{subject}` | short subject, a period comes first; if it repeats the document type, that is dropped | `2025`, `2026-Q1-Abschlag` |
| `{person}` | name from *People*; other spellings are merged | `Max` |
| `{extra}` | from the list *Extras* | `unterschrieben` |
| `{year}`, `{month}`, `{day}` | parts of the document date | `2026`, `09`, `25` |

**No spaces or special characters** (`compact_filenames`): within a part, spaces and `_ , ; : / | –` become `-`; other characters such as `& + # ( ) ' ! ?` and dots are dropped. No name gets longer than *Advanced › Maximum length* (`max_filename_length`, default 80, without the extension); the subject is shortened first, then the sender.

**Person only for documents in their own folders** (`person_from_folder`): a bank statement in `Bank/2026` gets no person, a payslip in `Gehalt/2026_Max` does.

Single folders and everything below them can have their own template (*Advanced › Custom template per folder*, `[[folder_templates]]`). Program files such as tax or key files keep their name (*Advanced › Never rename*, `keep_name_extensions`).

## Properties for Windows Search

With *Settings › Archive › Advanced › Write title and tags into the file for Windows Search* (`write_tags`, on by default), Scrinium writes these details into the file after filing:

| Property | Content | Example |
|---|---|---|
| Title | document type, sender and subject | `Steuerbescheid Finanzamt – 2025` |
| Subject | subject | `2025` |
| Tags | document type, sender, person, year, folder topics, extra | `Steuerbescheid; Finanzamt; 2025; Steuern` |
| Comments | summary by the AI | |

- **PDF:** info dictionary and XMP, appended as an incremental update – the existing content stays byte for byte. Signed and encrypted PDFs stay unchanged.
- **Word, Excel, PowerPoint:** document properties (`docProps/core.xml`).
- **JPG, PNG, TIFF, WebP:** through the Windows property system (on Windows only).
- Existing tags stay, new ones are added. Other formats stay unchanged.

In Explorer the details are under *Properties › Details*. Search with `tags:Steuerbescheid`, for example, or simply the sender.
