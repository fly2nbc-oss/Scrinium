# How Scrinium chooses a folder

Scrinium files only into folders that **already exist** (and continues year series). Three things decide where a document goes, in this order:

1. **Special rules** – unique features such as an account or personnel number, which also set fields of the file name.
2. **Keywords** (*Recognised by*) – words by which a folder can be recognised without doubt.
3. **The AI** – it reads the document and chooses based on the **folder descriptions**.

Folder descriptions are usually enough. You only need keywords and special rules for cases the AI mixes up.

## The AI chooses in steps

1. **Understand:** The AI reads the text and the page image and describes the document in two or three sentences: what it is, from whom, to whom, what it is about.
2. **Area:** From this description it chooses a folder on the top level. It sees every folder with its description and the names of its subfolders.
3. **Folder:** Level by level it chooses a subfolder until none fits better. Scrinium picks the year itself (see [Year folders](#year-folders)).
4. **File name:** Finally it sets date, document type, sender, subject, person and extra, with the chosen folder as context.

*Why?* under *Destination* shows why the AI chose a folder. With `llm.folder_steps = false`, Scrinium asks for everything in a single request.

## Folder descriptions

Under *Settings › Archive* each folder has a *Description for the AI* (`[folder_notes]`). Note briefly what belongs in it, above all on the top level:

- `Versicherungen` – “All letters from insurers: policies, premium invoices, cancellations”
- `Wohnung/Nebenkosten` – “Electricity, water, waste, heating”

The *Instructions for the AI* under *Settings › File names* only affect the details in the name, not the folder.

## Keywords (“Recognised by”)

The column **Recognised by** holds unique features such as bank or insurer names and account or contract numbers (`[folder_words]`). They decide before the AI, level by level:

| Notation | Meaning |
|---|---|
| `Haftpflicht, Police` | one of them is enough (comma = or) |
| `Grundsteuer & Musterstadt` | both words must occur (` & ` with spaces = and) |
| `-Kontoauszug` | negative word: if it occurs, the keywords of this folder and its subfolders do not count for it – the AI decides |
| `Versicherungs*` | longer words too (`Versicherungsschein`) |

- Scrinium compares whole words and ignores upper and lower case and umlauts.
- If exactly one folder on a level matches, Scrinium takes it without the AI. If several or none match, the AI chooses and sees the matches. So keywords can decide, but they cannot block a folder.
- Keywords of a subfolder also count for its parent folders.
- For a series of year folders, put `{year}` in the key: `"Arbeit/Gehalt/{year}_Max" = "Musterfirma"`.
- Without the AI, keywords choose the folder on their own as far as they are unique.

When you hover over the keywords, Scrinium explains in words how it reads them.

## Special rules

Under *Settings › Special rules* (`[[rules]]`). They take precedence over keywords and the AI; **the first matching rule wins**, and the arrows change the order. *Test special rules on a text* shows which rule matches a text. Rules compare text ignoring upper and lower case and umlauts; unlike keywords, a rule word also matches inside longer words.

| Field | Key | Meaning |
|---|---|---|
| *Keywords* | `keywords` | one of them is enough; with *All keywords must occur* (`match_all = true`) all must occur |
| *Must also contain* | `require_keywords` | all of these must also occur, e.g. your home town |
| *Negative words* | `exclude_keywords` | if one of them occurs, the rule does not apply (e.g. “Kontoauszug”) |
| *Target folder* | `folder` | `{year}` = reference year; `{month}` and `{day}` also work |
| *Sets* | `doctype`, `sender`, `person` | set these fields (optional) |
| switch (*Rule is on* / *Rule is off*) | `enabled` | a switched-off rule is kept but not used |

## Year folders

The **reference year** is a period at the start of the subject (`2025` for a tax assessment), otherwise the year of the document date.

- If the year folder already exists, Scrinium takes it – also in groups such as `202x/2022_Max`.
- Scrinium creates a new year folder only if it continues a series that reaches at least the previous year. So an old date in the text (such as a date of birth) does not create a folder `1974`.
- If a folder has only year subfolders, the document goes into the matching year.

Without a tick in the column *Used · Folder*, a folder and its subfolders are left out (`exclude`). This keeps the list for the AI short. For a collection folder whose subfolders the AI should not choose one by one, remove the tick from the subfolders.

## Learning from decisions

On every filing, Scrinium writes one line to `decisions.jsonl` next to `config.toml`: the suggestion for folder and fields, where it came from (AI, keyword, rule, text …), and what was filed. An undo gets a line of its own, and the filing no longer counts. The recognised text is not stored, only the target path. You can switch this off under *Settings › Archive › Advanced › Record decisions* (`log_decisions`).

`scrinium --report` sums up the file (or another file given as argument):

- how often the suggestion for each field was kept, split by source (`AI 30/32 · Keyword 8/9`)
- which changes repeat, such as `3×  Versicherungen/Haftpflicht → Versicherungen/Hausrat   [AI]`

A folder the AI often misses needs a sharper description or a keyword. A wrong `[Keyword]` points to a word that is too broad and lacks a negative word.

## Set up with AI

Under *Settings › Set up with AI* you describe your archive in your own words or load an existing description (Markdown or text). The local AI suggests from it:

- file name template and date format
- document types, extras, people with their spellings, short names for senders
- descriptions of the existing folders and instructions for the AI
- rules for unique features

You can apply each section on its own; next to the title you see what would be new and what would be replaced. Scrinium reports folders that do not exist and leaves rules for them switched off. Nothing is stored until you click *Save*.
