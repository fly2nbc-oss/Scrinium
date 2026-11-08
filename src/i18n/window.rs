//! Main window (`app.rs`): document list, buttons above it, status bar, messages, confirmations and the window
//! after scanning.

pub struct Texts {
    // Startup
    pub defaults_active: fn(&str) -> String,

    // Adding documents to the queue
    pub queued: fn(usize, &str) -> String,
    pub skipped: fn(usize) -> String,
    pub already_listed: fn(usize) -> String,
    pub subfolders_left_out: fn(usize) -> String,
    pub worker_stopped: &'static str,
    pub setup_ready: &'static str,

    // Filing and undo
    pub tags_written: &'static str,
    pub tags_skipped: fn(&str) -> String,
    pub tags_failed: fn(&str) -> String,
    pub journal_failed: fn(&str) -> String,
    pub pages_turned: &'static str,
    pub turn_unsupported: &'static str,
    pub turn_skipped: fn(&str) -> String,
    pub turn_failed: fn(&str) -> String,
    pub filed_toast: fn(&str) -> String,
    pub file_failed: &'static str,
    pub undo_copy_removed: fn(&str) -> String,
    pub undo_moved_back: fn(&str) -> String,
    pub undo_failed: fn(&str) -> String,
    pub undo_last_copy_removed: fn(&str) -> String,
    pub undo_last_moved_back: fn(&str) -> String,
    pub analysis_failed_warning: fn(&str) -> String,

    // Scanning
    pub scanner_opening: &'static str,
    pub scan_cancelled: &'static str,
    pub scan_error: fn(&str) -> String,
    pub scan_dir_failed: fn(&str, &str) -> String,
    pub scan_preparing: &'static str,
    pub preparation_cancelled: &'static str,
    pub scan_saved: fn(&str) -> String,
    pub scan_not_saved: fn(&str) -> String,

    // Settings
    pub setup_waiting: &'static str,
    pub config_reloaded: &'static str,
    pub settings_saved: &'static str,

    // AI server
    pub models_available: fn(&str, usize) -> String,
    pub no_model_loaded: &'static str,

    // File picker
    pub pick_files_title: &'static str,
    pub documents: &'static str,
    pub all_files: &'static str,
    pub pick_folder_title: &'static str,

    // AI state in the AI tab
    pub ai_status_checking: &'static str,
    pub ai_status_off: &'static str,

    // Gear button
    pub settings: &'static str,
    pub settings_newer: fn(&str) -> String,

    // AI in the status bar
    pub ai_download: fn(f32) -> String,
    pub ai_off: &'static str,
    pub ai_off_hint: &'static str,
    pub ai_checking: &'static str,
    pub ai_model: fn(&str) -> String,
    pub ai_no_model: &'static str,
    pub ai_unreachable: &'static str,
    pub ai_meanwhile: fn(&str) -> String,
    pub ai_click_settings: &'static str,
    pub ai_click_check: &'static str,

    // Status bar
    pub back_to_documents: &'static str,
    pub show_list: &'static str,
    pub hide_list: &'static str,
    /// To review, in progress, waiting, paused?, filed.
    pub status_counts: fn(usize, usize, usize, bool, usize) -> String,
    pub archive_status: fn(&str, usize) -> String,
    pub archive_missing: fn(&str) -> String,
    pub click_opens_folder: fn(&str) -> String,
    pub undo_copy_hint_key: &'static str,
    pub undo_move_hint_key: &'static str,
    pub undo: &'static str,
    pub last_filed: fn(&str) -> String,

    // List
    pub files_button: &'static str,
    pub files_hint: &'static str,
    pub folder_button: &'static str,
    pub folder_hint: &'static str,
    pub scanning: &'static str,
    pub preparing: &'static str,
    pub scan: &'static str,
    pub scan_hint: &'static str,
    /// Scan disabled.
    pub scan_busy: &'static str,
    /// Label in front of the selection below the *Scan* button.
    pub scan_as: &'static str,
    pub queue_counts: fn(usize, usize) -> String,
    pub resume: &'static str,
    pub pause: &'static str,
    pub pause_hint: &'static str,
    /// Pause disabled.
    pub queue_empty: &'static str,
    pub hide_done: &'static str,
    pub hide_done_hint: &'static str,
    /// *Remove waiting* disabled.
    pub nothing_waiting: &'static str,
    /// *Hide done* disabled.
    pub nothing_filed: &'static str,
    pub remove_waiting: &'static str,
    pub paused_note: &'static str,
    pub row_waiting: &'static str,
    pub row_next: &'static str,
    pub row_check: fn(&str, &str) -> String,
    pub row_filed: fn(&str) -> String,
    pub earlier: &'static str,
    pub later: &'static str,
    pub reanalyze: &'static str,
    pub remove_hint: &'static str,

    // Empty list
    pub drop_here: &'static str,
    pub supported: &'static str,
    pub steps: [&'static str; 3],
    pub choose_files: &'static str,
    pub choose_folder: &'static str,
    pub scan_dots: &'static str,
    pub archive_absent: fn(&str) -> String,
    pub choose_archive: &'static str,
    pub create_here: &'static str,
    pub create_failed: fn(&str) -> String,
    /// Archive, folders, of those for the AI, rules.
    pub archive_summary: fn(&str, usize, usize, usize) -> String,
    pub no_model_yet: &'static str,
    pub server_unreachable: &'static str,
    pub set_up_ai: &'static str,
    pub ai_disabled: &'static str,
    pub new_here: &'static str,

    // Document
    pub select_document: &'static str,
    pub from_dir: fn(&str) -> String,
    pub waiting_slot: fn(usize, usize) -> String,
    pub waiting_in_queue: &'static str,
    pub analysed_next: &'static str,
    pub queue_paused: &'static str,
    pub one_by_one: &'static str,
    /// Position, count, seconds since the start.
    pub working_slot: fn(usize, usize, Option<u64>) -> String,
    pub analysis_failed: &'static str,
    pub try_again: &'static str,
    pub file_anyway: &'static str,
    pub file_anyway_hint: &'static str,
    pub filed_as_copy: &'static str,
    pub filed: &'static str,
    pub show_in_file_manager: &'static str,
    pub undo_move_hint: &'static str,

    // Please review: reasons
    pub no_matching_folder: &'static str,
    pub uncertain: &'static str,
    pub archive_unreachable: &'static str,
    pub new_folder: &'static str,
    pub note: &'static str,

    // Confirmation before discarding
    pub remove_from_list: &'static str,
    pub discard_title: &'static str,
    pub discard_text: fn(&str) -> String,
    pub back: &'static str,

    // Other
    pub archive: &'static str,
    pub drop_release: &'static str,
    /// Tooltip on a notice above the content.
    pub notice_close: &'static str,
}

pub fn t() -> &'static Texts {
    super::pick(&DE, &EN, &FR, &ES, &IT)
}

pub static DE: Texts = Texts {
    defaults_active: |e| format!("{e} – Standardwerte aktiv."),

    queued: |n, name| match n {
        1 => format!("„{name}“ in der Warteschlange"),
        n => format!("{n} Dokumente in der Warteschlange"),
    },
    skipped: |n| format!("{n} übersprungen (Systemdatei oder nicht lesbar)"),
    already_listed: |n| format!("{n} schon in der Liste"),
    subfolders_left_out: |n| format!("{n} Unterordner nicht mitgenommen"),
    worker_stopped: "Analyse-Thread ist beendet",
    setup_ready: "Vorschlag für die Einstellungen ist fertig",

    tags_written: " · Markierungen für die Windows-Suche gespeichert",
    tags_skipped: |why| format!(" · Markierungen nicht geschrieben ({why})"),
    tags_failed: |e| format!(" · Markierungen nicht geschrieben: {e}"),
    journal_failed: |e| format!(" · Entscheidung nicht aufgezeichnet: {e}"),
    pages_turned: " · Seiten gedreht gespeichert",
    turn_unsupported: " · Drehung nur in der Vorschau (dieses Format dreht Scrinium nicht)",
    turn_skipped: |why| format!(" · Seiten nicht gedreht ({why})"),
    turn_failed: |e| format!(" · Seiten nicht gedreht: {e}"),
    filed_toast: |path| format!("Abgelegt: {path}"),
    file_failed: "Ablegen fehlgeschlagen – Grund steht im Formular",
    undo_copy_removed: |name| format!("Rückgängig: {name} – die Kopie in der Ablage ist entfernt"),
    undo_moved_back: |name| format!("Rückgängig: {name} – liegt wieder am alten Ort"),
    undo_failed: |e| format!("Rückgängig nicht möglich: {e}"),
    undo_last_copy_removed: |name| format!("Rückgängig: Kopie von {name} entfernt"),
    undo_last_moved_back: |name| format!("Rückgängig: {name} liegt wieder am alten Ort und ist neu in der Liste"),
    analysis_failed_warning: |e| format!("Analyse fehlgeschlagen: {e}"),

    scanner_opening: "Der Scanner wird geöffnet …",
    scan_cancelled: "Der Scan wurde abgebrochen.",
    scan_error: |e| format!("Scannen: {e}"),
    scan_dir_failed: |dir, e| format!("Scan nicht gespeichert: {dir} ließ sich nicht anlegen ({e})"),
    scan_preparing: "Scan aufbereiten …",
    preparation_cancelled: "Die Aufbereitung wurde abgebrochen.",
    scan_saved: |path| format!("Scan gespeichert: {path}"),
    scan_not_saved: |e| format!("Scan nicht gespeichert: {e}"),

    setup_waiting: "Warte auf die KI …",
    config_reloaded: "Konfiguration neu geladen",
    settings_saved: "Einstellungen gespeichert",

    models_available: |url, n| format!("{url} · {n} Modell(e) verfügbar"),
    no_model_loaded: "Kein Modell geladen",

    pick_files_title: "Dokumente auswählen",
    documents: "Dokumente",
    all_files: "Alle Dateien",
    pick_folder_title: "Ordner mit Dokumenten",

    ai_status_checking: "wird geprüft …",
    ai_status_off: "aus",

    settings: "Einstellungen",
    settings_newer: |v| format!("Einstellungen – Scrinium {v} ist verfügbar"),

    ai_download: |p| format!("KI-Download {p:.0} %"),
    ai_off: "KI aus",
    ai_off_hint: "Nur Texterkennung, Stichwörter und Regeln",
    ai_checking: "KI wird geprüft …",
    ai_model: |m| format!("KI: {m}"),
    ai_no_model: "KI: kein Modell",
    ai_unreachable: "KI nicht erreichbar",
    ai_meanwhile: |e| format!("{e}\nBis dahin: Texterkennung, Stichwörter und Regeln."),
    ai_click_settings: "Klicken öffnet die KI-Einstellungen",
    ai_click_check: "Klicken prüft erneut",

    back_to_documents: "Zurück zu den Dokumenten",
    show_list: "Liste zeigen",
    hide_list: "Liste ausblenden – mehr Platz für Dokument und Formular",
    status_counts: |open, running, waiting, paused, filed| {
        let pause = if paused { " · pausiert" } else { "" };
        format!("{open} zu prüfen · {running} in Arbeit · {waiting} warten{pause} · {filed} abgelegt")
    },
    archive_status: |root, n| format!("Ablage: {root} · {n} Ordner"),
    archive_missing: |root| format!("Ablage fehlt: {root}"),
    click_opens_folder: |path| format!("{path}\nKlicken öffnet den Ordner"),
    undo_copy_hint_key: "Kopie in der Ablage entfernen (Strg+Z)",
    undo_move_hint_key: "Datei zurück an den alten Ort (Strg+Z)",
    undo: "Rückgängig",
    last_filed: |path| format!("Zuletzt: {path}"),

    files_button: "Dateien",
    files_hint: "Eine oder mehrere Dateien in die Warteschlange",
    folder_button: "Ordner",
    folder_hint: "Alle Dokumente eines Ordners in die Warteschlange",
    scanning: "Scannt …",
    preparing: "Bereitet auf …",
    scan: "Scannen",
    scan_hint: "Vom Scanner holen – mehrere Seiten werden ein PDF",
    scan_busy: "Ein Scan läuft gerade",
    scan_as: "Scannen als",
    queue_counts: |open, queued| format!("{open} zu prüfen · {queued} in der Warteschlange"),
    resume: "Weiter",
    pause: "Pause",
    pause_hint: "Das laufende Dokument wird noch fertig",
    queue_empty: "Kein Dokument in der Warteschlange",
    hide_done: "Erledigte ausblenden",
    hide_done_hint: "Die letzte Ablage lässt sich danach weiter zurücknehmen",
    nothing_waiting: "Kein Dokument wartet",
    nothing_filed: "Noch nichts abgelegt",
    remove_waiting: "Wartende entfernen",
    paused_note: "Pausiert – das laufende Dokument wird noch fertig.",
    row_waiting: "wartet",
    row_next: "als Nächstes",
    row_check: |why, folder| format!("bitte prüfen: {why} · → {folder}"),
    row_filed: |name| format!("abgelegt: {name}"),
    earlier: "Früher bearbeiten",
    later: "Später bearbeiten",
    reanalyze: "Neu analysieren",
    remove_hint: "Aus der Liste nehmen (die Datei bleibt, wo sie ist)",

    drop_here: "Dokumente hierher ziehen",
    supported: "Scans und PDFs, Fotos (JPG, PNG), Word, Excel, PowerPoint und LibreOffice, Markdown und Text – \
                einzeln oder als ganzer Ordner. Andere Dateien (z. B. ZIP) lassen sich nach Dateiname ablegen.",
    steps: [
        "Scrinium liest jedes Dokument lokal und schlägt Datum, Art, Absender, Ordner und Namen vor.",
        "Du prüfst rechts den Vorschlag und änderst, was nicht passt.",
        "„Ablegen“ (Strg+Enter) verschiebt die Datei; „Rückgängig“ holt sie zurück.",
    ],
    choose_files: "Dateien auswählen …",
    choose_folder: "Ordner auswählen …",
    scan_dots: "Scannen …",
    archive_absent: |root| format!("Ablageordner {root} existiert noch nicht."),
    choose_archive: "Ablageordner wählen …",
    create_here: "Hier anlegen",
    create_failed: |e| format!("Ordner konnte nicht angelegt werden: {e}"),
    archive_summary: |root, folders, offered, rules| {
        format!("Ablage {root} · {folders} Ordner, {offered} davon für die KI · {rules} Regeln")
    },
    no_model_yet: "Noch kein KI-Modell geladen – bis dahin helfen Texterkennung, Stichwörter und Regeln.",
    server_unreachable: "Der KI-Server ist nicht erreichbar – bis dahin helfen Texterkennung, Stichwörter und Regeln.",
    set_up_ai: "KI einrichten",
    ai_disabled: "KI ist ausgeschaltet – Analyse über Texterkennung, Stichwörter und Regeln.",
    new_here: "Neu hier? Ablage beschreiben und mit KI einrichten …",

    select_document: "Dokument links auswählen",
    from_dir: |dir| format!("aus {dir}"),
    waiting_slot: |i, n| format!("Wartet in der Warteschlange ({i} von {n})"),
    waiting_in_queue: "Wartet in der Warteschlange",
    analysed_next: "Wird als Nächstes analysiert …",
    queue_paused: "Die Warteschlange ist pausiert.",
    one_by_one: "Die Dokumente werden nacheinander bearbeitet.",
    working_slot: |i, n, secs| {
        let time = secs.map(|s| format!(" · seit {s} s")).unwrap_or_default();
        format!("Dokument {i} von {n} · alles läuft lokal{time}")
    },
    analysis_failed: "Analyse fehlgeschlagen",
    try_again: "Erneut versuchen",
    file_anyway: "Trotzdem ablegen …",
    file_anyway_hint: "Name und Ordner selbst eintragen",
    filed_as_copy: "Als Kopie abgelegt",
    filed: "Abgelegt",
    show_in_file_manager: "Im Dateimanager zeigen",
    undo_move_hint: "Datei zurück an den alten Ort",

    no_matching_folder: "kein passender Ordner",
    uncertain: "unsicher",
    archive_unreachable: "Ablage nicht erreichbar",
    new_folder: "neuer Ordner",
    note: "Hinweis",

    remove_from_list: "Aus der Liste nehmen",
    discard_title: "Eingaben verwerfen?",
    discard_text: |what| format!("Du hast den Vorschlag geändert. „{what}“ verwirft diese Änderungen."),
    back: "Zurück",

    archive: "Ablage",
    drop_release: "Loslassen zum Analysieren",
    notice_close: "Klicken schließt den Hinweis",
};

pub static EN: Texts = Texts {
    defaults_active: |e| format!("{e} – using default values."),

    queued: |n, name| match n {
        1 => format!("“{name}” in the queue"),
        n => format!("{n} documents in the queue"),
    },
    skipped: |n| format!("{n} skipped (system file or unreadable)"),
    already_listed: |n| format!("{n} already in the list"),
    subfolders_left_out: |n| match n {
        1 => "1 subfolder not included".to_string(),
        n => format!("{n} subfolders not included"),
    },
    worker_stopped: "The analysis thread has stopped",
    setup_ready: "Suggested settings are ready",

    tags_written: " · Tags saved for Windows Search",
    tags_skipped: |why| format!(" · Tags not written ({why})"),
    tags_failed: |e| format!(" · Tags not written: {e}"),
    journal_failed: |e| format!(" · Decision not recorded: {e}"),
    pages_turned: " · pages saved rotated",
    turn_unsupported: " · rotation only in the preview (Scrinium does not rotate this format)",
    turn_skipped: |why| format!(" · pages not rotated ({why})"),
    turn_failed: |e| format!(" · pages not rotated: {e}"),
    filed_toast: |path| format!("Filed: {path}"),
    file_failed: "Filing failed – the reason is shown in the form",
    undo_copy_removed: |name| format!("Undone: {name} – the copy in the archive has been removed"),
    undo_moved_back: |name| format!("Undone: {name} – back where it was"),
    undo_failed: |e| format!("Cannot undo: {e}"),
    undo_last_copy_removed: |name| format!("Undone: copy of {name} removed"),
    undo_last_moved_back: |name| format!("Undone: {name} is back where it was and in the list again"),
    analysis_failed_warning: |e| format!("Analysis failed: {e}"),

    scanner_opening: "Opening the scanner …",
    scan_cancelled: "The scan was cancelled.",
    scan_error: |e| format!("Scan: {e}"),
    scan_dir_failed: |dir, e| format!("Scan not saved: could not create {dir} ({e})"),
    scan_preparing: "Processing the scan …",
    preparation_cancelled: "Processing was cancelled.",
    scan_saved: |path| format!("Scan saved: {path}"),
    scan_not_saved: |e| format!("Scan not saved: {e}"),

    setup_waiting: "Waiting for the AI …",
    config_reloaded: "Configuration reloaded",
    settings_saved: "Settings saved",

    models_available: |url, n| match n {
        1 => format!("{url} · 1 model available"),
        n => format!("{url} · {n} models available"),
    },
    no_model_loaded: "No model loaded",

    pick_files_title: "Choose documents",
    documents: "Documents",
    all_files: "All files",
    pick_folder_title: "Folder with documents",

    ai_status_checking: "checking …",
    ai_status_off: "off",

    settings: "Settings",
    settings_newer: |v| format!("Settings – Scrinium {v} is available"),

    ai_download: |p| format!("AI download {p:.0}%"),
    ai_off: "AI off",
    ai_off_hint: "Text recognition, keywords and rules only",
    ai_checking: "Checking AI …",
    ai_model: |m| format!("AI: {m}"),
    ai_no_model: "AI: no model",
    ai_unreachable: "AI unreachable",
    ai_meanwhile: |e| format!("{e}\nUntil then: text recognition, keywords and rules."),
    ai_click_settings: "Click to open the AI settings",
    ai_click_check: "Click to check again",

    back_to_documents: "Back to the documents",
    show_list: "Show list",
    hide_list: "Hide list – more room for document and form",
    status_counts: |open, running, waiting, paused, filed| {
        let pause = if paused { " · paused" } else { "" };
        format!("{open} to check · {running} in progress · {waiting} waiting{pause} · {filed} filed")
    },
    archive_status: |root, n| match n {
        1 => format!("Archive: {root} · 1 folder"),
        n => format!("Archive: {root} · {n} folders"),
    },
    archive_missing: |root| format!("Archive missing: {root}"),
    click_opens_folder: |path| format!("{path}\nClick to open the folder"),
    undo_copy_hint_key: "Remove the copy from the archive (Ctrl+Z)",
    undo_move_hint_key: "Move the file back to where it was (Ctrl+Z)",
    undo: "Undo",
    last_filed: |path| format!("Last: {path}"),

    files_button: "Files",
    files_hint: "Add one or more files to the queue",
    folder_button: "Folder",
    folder_hint: "Add all documents in a folder to the queue",
    scanning: "Scanning …",
    preparing: "Processing …",
    scan: "Scan",
    scan_hint: "Get from the scanner – several pages become one PDF",
    scan_busy: "A scan is in progress",
    scan_as: "Scan as",
    queue_counts: |open, queued| format!("{open} to check · {queued} in the queue"),
    resume: "Resume",
    pause: "Pause",
    pause_hint: "The current document will still be finished",
    queue_empty: "No document in the queue",
    hide_done: "Hide filed",
    hide_done_hint: "The last filing can still be undone afterwards",
    nothing_waiting: "No document is waiting",
    nothing_filed: "Nothing filed yet",
    remove_waiting: "Clear queue",
    paused_note: "Paused – the current document will still be finished.",
    row_waiting: "waiting",
    row_next: "next up",
    row_check: |why, folder| format!("please check: {why} · → {folder}"),
    row_filed: |name| format!("filed: {name}"),
    earlier: "Process earlier",
    later: "Process later",
    reanalyze: "Analyse again",
    remove_hint: "Remove from the list (the file stays where it is)",

    drop_here: "Drag documents here",
    supported: "Scans and PDFs, photos (JPG, PNG), Word, Excel, PowerPoint and LibreOffice, Markdown and text – one \
                by one or as a whole folder. Other files (e.g. ZIP) can be filed by file name.",
    steps: [
        "Scrinium reads each document locally and suggests date, type, sender, folder and name.",
        "You check the suggestion on the right and change whatever doesn't fit.",
        "“File” (Ctrl+Enter) moves the file; “Undo” brings it back.",
    ],
    choose_files: "Choose files …",
    choose_folder: "Choose folder …",
    scan_dots: "Scan …",
    archive_absent: |root| format!("Archive folder {root} does not exist yet."),
    choose_archive: "Choose archive folder …",
    create_here: "Create here",
    create_failed: |e| format!("Could not create the folder: {e}"),
    archive_summary: |root, folders, offered, rules| {
        let folders = match folders {
            1 => "1 folder".to_string(),
            n => format!("{n} folders"),
        };
        let rules = match rules {
            1 => "1 rule".to_string(),
            n => format!("{n} rules"),
        };
        format!("Archive {root} · {folders}, {offered} of them for the AI · {rules}")
    },
    no_model_yet: "No AI model loaded yet – until then, text recognition, keywords and rules help out.",
    server_unreachable: "The AI server cannot be reached – until then, text recognition, keywords and rules help out.",
    set_up_ai: "Set up AI",
    ai_disabled: "AI is switched off – analysis uses text recognition, keywords and rules.",
    new_here: "New here? Describe your archive and set up with AI …",

    select_document: "Select a document on the left",
    from_dir: |dir| format!("from {dir}"),
    waiting_slot: |i, n| format!("Waiting in the queue ({i} of {n})"),
    waiting_in_queue: "Waiting in the queue",
    analysed_next: "Will be analysed next …",
    queue_paused: "The queue is paused.",
    one_by_one: "Documents are processed one after another.",
    working_slot: |i, n, secs| {
        let time = secs.map(|s| format!(" · for {s} s")).unwrap_or_default();
        format!("Document {i} of {n} · everything runs locally{time}")
    },
    analysis_failed: "Analysis failed",
    try_again: "Try again",
    file_anyway: "File anyway …",
    file_anyway_hint: "Enter name and folder yourself",
    filed_as_copy: "Filed as a copy",
    filed: "Filed",
    show_in_file_manager: "Show in file manager",
    undo_move_hint: "Move the file back to where it was",

    no_matching_folder: "no matching folder",
    uncertain: "uncertain",
    archive_unreachable: "archive unreachable",
    new_folder: "new folder",
    note: "note",

    remove_from_list: "Remove from list",
    discard_title: "Discard changes?",
    discard_text: |what| format!("You have changed the suggestion. “{what}” discards these changes."),
    back: "Back",

    archive: "Archive",
    drop_release: "Release to analyse",
    notice_close: "Click to close",
};

pub static FR: Texts = Texts {
    defaults_active: |e| format!("{e} – valeurs par défaut actives."),

    queued: |n, name| match n {
        1 => format!("« {name} » dans la file d'attente"),
        n => format!("{n} documents dans la file d'attente"),
    },
    skipped: |n| match n {
        0 | 1 => format!("{n} ignoré (fichier système ou illisible)"),
        n => format!("{n} ignorés (fichiers système ou illisibles)"),
    },
    already_listed: |n| format!("{n} déjà dans la liste"),
    subfolders_left_out: |n| match n {
        0 | 1 => format!("{n} sous-dossier non inclus"),
        n => format!("{n} sous-dossiers non inclus"),
    },
    worker_stopped: "Le processus d'analyse s'est arrêté",
    setup_ready: "La proposition de paramètres est prête",

    tags_written: " · Mots-clés enregistrés pour la recherche Windows",
    tags_skipped: |why| format!(" · Mots-clés non écrits ({why})"),
    tags_failed: |e| format!(" · Mots-clés non écrits : {e}"),
    journal_failed: |e| format!(" · Décision non enregistrée : {e}"),
    pages_turned: " · pages enregistrées pivotées",
    turn_unsupported: " · rotation seulement dans l'aperçu (Scrinium ne fait pas pivoter ce format)",
    turn_skipped: |why| format!(" · pages non pivotées ({why})"),
    turn_failed: |e| format!(" · pages non pivotées : {e}"),
    filed_toast: |path| format!("Classé : {path}"),
    file_failed: "Échec du classement – la raison figure dans le formulaire",
    undo_copy_removed: |name| format!("Annulé : {name} – la copie dans l'archive a été supprimée"),
    undo_moved_back: |name| format!("Annulé : {name} – de retour à son ancien emplacement"),
    undo_failed: |e| format!("Impossible d'annuler : {e}"),
    undo_last_copy_removed: |name| format!("Annulé : copie de {name} supprimée"),
    undo_last_moved_back: |name| {
        format!("Annulé : {name} est de retour à son ancien emplacement et à nouveau dans la liste")
    },
    analysis_failed_warning: |e| format!("Échec de l'analyse : {e}"),

    scanner_opening: "Ouverture du scanner …",
    scan_cancelled: "La numérisation a été annulée.",
    scan_error: |e| format!("Numérisation : {e}"),
    scan_dir_failed: |dir, e| format!("Numérisation non enregistrée : impossible de créer {dir} ({e})"),
    scan_preparing: "Traitement de la numérisation …",
    preparation_cancelled: "Le traitement a été interrompu.",
    scan_saved: |path| format!("Numérisation enregistrée : {path}"),
    scan_not_saved: |e| format!("Numérisation non enregistrée : {e}"),

    setup_waiting: "En attente de l'IA …",
    config_reloaded: "Configuration rechargée",
    settings_saved: "Paramètres enregistrés",

    models_available: |url, n| match n {
        0 | 1 => format!("{url} · {n} modèle disponible"),
        n => format!("{url} · {n} modèles disponibles"),
    },
    no_model_loaded: "Aucun modèle chargé",

    pick_files_title: "Choisir des documents",
    documents: "Documents",
    all_files: "Tous les fichiers",
    pick_folder_title: "Dossier de documents",

    ai_status_checking: "vérification …",
    ai_status_off: "désactivée",

    settings: "Paramètres",
    settings_newer: |v| format!("Paramètres – Scrinium {v} est disponible"),

    ai_download: |p| format!("Téléchargement IA {p:.0} %"),
    ai_off: "IA désactivée",
    ai_off_hint: "Uniquement reconnaissance de texte, mots-clés et règles",
    ai_checking: "Vérification de l'IA …",
    ai_model: |m| format!("IA : {m}"),
    ai_no_model: "IA : aucun modèle",
    ai_unreachable: "IA injoignable",
    ai_meanwhile: |e| format!("{e}\nEn attendant : reconnaissance de texte, mots-clés et règles."),
    ai_click_settings: "Cliquez pour ouvrir les paramètres de l'IA",
    ai_click_check: "Cliquez pour vérifier à nouveau",

    back_to_documents: "Retour aux documents",
    show_list: "Afficher la liste",
    hide_list: "Masquer la liste – plus de place pour le document et le formulaire",
    status_counts: |open, running, waiting, paused, filed| {
        let pause = if paused { " · en pause" } else { "" };
        let filed = match filed {
            0 | 1 => format!("{filed} classé"),
            n => format!("{n} classés"),
        };
        format!("{open} à vérifier · {running} en cours · {waiting} en attente{pause} · {filed}")
    },
    archive_status: |root, n| match n {
        0 | 1 => format!("Archive : {root} · {n} dossier"),
        n => format!("Archive : {root} · {n} dossiers"),
    },
    archive_missing: |root| format!("Archive introuvable : {root}"),
    click_opens_folder: |path| format!("{path}\nCliquez pour ouvrir le dossier"),
    undo_copy_hint_key: "Supprimer la copie dans l'archive (Ctrl+Z)",
    undo_move_hint_key: "Remettre le fichier à son ancien emplacement (Ctrl+Z)",
    undo: "Annuler",
    last_filed: |path| format!("Dernier : {path}"),

    files_button: "Fichiers",
    files_hint: "Ajouter un ou plusieurs fichiers à la file d'attente",
    folder_button: "Dossier",
    folder_hint: "Ajouter tous les documents d'un dossier à la file d'attente",
    scanning: "Numérise …",
    preparing: "Traitement …",
    scan: "Numériser",
    scan_hint: "Récupérer depuis le scanner – plusieurs pages forment un seul PDF",
    scan_busy: "Une numérisation est en cours",
    scan_as: "Numériser en",
    queue_counts: |open, queued| format!("{open} à vérifier · {queued} en file d'attente"),
    resume: "Reprendre",
    pause: "Pause",
    pause_hint: "Le document en cours sera tout de même terminé",
    queue_empty: "Aucun document dans la file d'attente",
    hide_done: "Masquer les classés",
    hide_done_hint: "Le dernier classement pourra toujours être annulé",
    nothing_waiting: "Aucun document en attente",
    nothing_filed: "Rien n'a encore été classé",
    remove_waiting: "Vider la file",
    paused_note: "En pause – le document en cours sera tout de même terminé.",
    row_waiting: "en attente",
    row_next: "à suivre",
    row_check: |why, folder| format!("à vérifier : {why} · → {folder}"),
    row_filed: |name| format!("classé : {name}"),
    earlier: "Traiter plus tôt",
    later: "Traiter plus tard",
    reanalyze: "Réanalyser",
    remove_hint: "Retirer de la liste (le fichier reste où il est)",

    drop_here: "Glissez des documents ici",
    supported: "Numérisations et PDF, photos (JPG, PNG), Word, Excel, PowerPoint et LibreOffice, Markdown et texte – \
                un par un ou tout un dossier. Les autres fichiers (p. ex. ZIP) peuvent être classés d'après leur nom \
                de fichier.",
    steps: [
        "Scrinium lit chaque document en local et propose la date, le type, l'expéditeur, le dossier et le nom.",
        "Vous vérifiez la proposition à droite et modifiez ce qui ne convient pas.",
        "« Classer » (Ctrl+Entrée) déplace le fichier ; « Annuler » le remet en place.",
    ],
    choose_files: "Choisir des fichiers …",
    choose_folder: "Choisir un dossier …",
    scan_dots: "Numériser …",
    archive_absent: |root| format!("Le dossier d'archive {root} n'existe pas encore."),
    choose_archive: "Choisir le dossier d'archive …",
    create_here: "Créer ici",
    create_failed: |e| format!("Impossible de créer le dossier : {e}"),
    archive_summary: |root, folders, offered, rules| {
        let folders = match folders {
            0 | 1 => format!("{folders} dossier"),
            n => format!("{n} dossiers"),
        };
        let rules = match rules {
            0 | 1 => format!("{rules} règle"),
            n => format!("{n} règles"),
        };
        format!("Archive {root} · {folders}, dont {offered} pour l'IA · {rules}")
    },
    no_model_yet: "Aucun modèle d'IA chargé pour l'instant – en attendant, la reconnaissance de texte, les mots-clés \
                   et les règles prennent le relais.",
    server_unreachable: "Le serveur d'IA est injoignable – en attendant, la reconnaissance de texte, les mots-clés et \
                         les règles prennent le relais.",
    set_up_ai: "Configurer l'IA",
    ai_disabled: "L'IA est désactivée – analyse par reconnaissance de texte, mots-clés et règles.",
    new_here: "Nouveau ici ? Décrivez votre archive et configurez avec l'IA …",

    select_document: "Sélectionnez un document à gauche",
    from_dir: |dir| format!("de {dir}"),
    waiting_slot: |i, n| format!("En attente dans la file ({i} sur {n})"),
    waiting_in_queue: "En attente dans la file",
    analysed_next: "Sera analysé ensuite …",
    queue_paused: "La file d'attente est en pause.",
    one_by_one: "Les documents sont traités l'un après l'autre.",
    working_slot: |i, n, secs| {
        let time = secs.map(|s| format!(" · depuis {s} s")).unwrap_or_default();
        format!("Document {i} sur {n} · tout reste en local{time}")
    },
    analysis_failed: "Échec de l'analyse",
    try_again: "Réessayer",
    file_anyway: "Classer quand même …",
    file_anyway_hint: "Saisir vous-même le nom et le dossier",
    filed_as_copy: "Classé comme copie",
    filed: "Classé",
    show_in_file_manager: "Afficher dans l'explorateur",
    undo_move_hint: "Remettre le fichier à son ancien emplacement",

    no_matching_folder: "aucun dossier adapté",
    uncertain: "incertain",
    archive_unreachable: "archive inaccessible",
    new_folder: "nouveau dossier",
    note: "remarque",

    remove_from_list: "Retirer de la liste",
    discard_title: "Abandonner les modifications ?",
    discard_text: |what| format!("Vous avez modifié la proposition. « {what} » abandonne ces modifications."),
    back: "Retour",

    archive: "Archive",
    drop_release: "Relâchez pour analyser",
    notice_close: "Cliquez pour fermer",
};

pub static ES: Texts = Texts {
    defaults_active: |e| format!("{e} – se usan los valores predeterminados."),

    queued: |n, name| match n {
        1 => format!("«{name}» en la cola"),
        n => format!("{n} documentos en la cola"),
    },
    skipped: |n| match n {
        1 => "1 omitido (archivo del sistema o ilegible)".to_string(),
        n => format!("{n} omitidos (archivos del sistema o ilegibles)"),
    },
    already_listed: |n| format!("{n} ya en la lista"),
    subfolders_left_out: |n| match n {
        1 => "1 subcarpeta no incluida".to_string(),
        n => format!("{n} subcarpetas no incluidas"),
    },
    worker_stopped: "El proceso de análisis se ha detenido",
    setup_ready: "La propuesta de ajustes está lista",

    tags_written: " · Etiquetas guardadas para la búsqueda de Windows",
    tags_skipped: |why| format!(" · Etiquetas no escritas ({why})"),
    tags_failed: |e| format!(" · Etiquetas no escritas: {e}"),
    journal_failed: |e| format!(" · Decisión no registrada: {e}"),
    pages_turned: " · páginas guardadas giradas",
    turn_unsupported: " · giro solo en la vista previa (Scrinium no gira este formato)",
    turn_skipped: |why| format!(" · páginas no giradas ({why})"),
    turn_failed: |e| format!(" · páginas no giradas: {e}"),
    filed_toast: |path| format!("Archivado: {path}"),
    file_failed: "No se pudo archivar – el motivo está en el formulario",
    undo_copy_removed: |name| format!("Deshecho: {name} – se ha quitado la copia del archivo"),
    undo_moved_back: |name| format!("Deshecho: {name} – vuelve a estar en su sitio anterior"),
    undo_failed: |e| format!("No se puede deshacer: {e}"),
    undo_last_copy_removed: |name| format!("Deshecho: se ha quitado la copia de {name}"),
    undo_last_moved_back: |name| format!("Deshecho: {name} vuelve a estar en su sitio anterior y de nuevo en la lista"),
    analysis_failed_warning: |e| format!("Error en el análisis: {e}"),

    scanner_opening: "Abriendo el escáner …",
    scan_cancelled: "Se canceló el escaneo.",
    scan_error: |e| format!("Escaneo: {e}"),
    scan_dir_failed: |dir, e| format!("Escaneo no guardado: no se pudo crear {dir} ({e})"),
    scan_preparing: "Procesando el escaneo …",
    preparation_cancelled: "Se interrumpió el procesado.",
    scan_saved: |path| format!("Escaneo guardado: {path}"),
    scan_not_saved: |e| format!("Escaneo no guardado: {e}"),

    setup_waiting: "Esperando a la IA …",
    config_reloaded: "Configuración recargada",
    settings_saved: "Ajustes guardados",

    models_available: |url, n| match n {
        1 => format!("{url} · 1 modelo disponible"),
        n => format!("{url} · {n} modelos disponibles"),
    },
    no_model_loaded: "Ningún modelo cargado",

    pick_files_title: "Elegir documentos",
    documents: "Documentos",
    all_files: "Todos los archivos",
    pick_folder_title: "Carpeta con documentos",

    ai_status_checking: "comprobando …",
    ai_status_off: "desactivada",

    settings: "Ajustes",
    settings_newer: |v| format!("Ajustes – Scrinium {v} está disponible"),

    ai_download: |p| format!("Descarga de IA {p:.0} %"),
    ai_off: "IA desactivada",
    ai_off_hint: "Solo reconocimiento de texto, palabras clave y reglas",
    ai_checking: "Comprobando la IA …",
    ai_model: |m| format!("IA: {m}"),
    ai_no_model: "IA: sin modelo",
    ai_unreachable: "IA no disponible",
    ai_meanwhile: |e| format!("{e}\nMientras tanto: reconocimiento de texto, palabras clave y reglas."),
    ai_click_settings: "Haz clic para abrir los ajustes de IA",
    ai_click_check: "Haz clic para volver a comprobar",

    back_to_documents: "Volver a los documentos",
    show_list: "Mostrar la lista",
    hide_list: "Ocultar la lista – más espacio para el documento y el formulario",
    status_counts: |open, running, waiting, paused, filed| {
        let pause = if paused { " · en pausa" } else { "" };
        let filed = match filed {
            1 => "1 archivado".to_string(),
            n => format!("{n} archivados"),
        };
        format!("{open} por revisar · {running} en curso · {waiting} en espera{pause} · {filed}")
    },
    archive_status: |root, n| match n {
        1 => format!("Archivo: {root} · 1 carpeta"),
        n => format!("Archivo: {root} · {n} carpetas"),
    },
    archive_missing: |root| format!("Falta el archivo: {root}"),
    click_opens_folder: |path| format!("{path}\nHaz clic para abrir la carpeta"),
    undo_copy_hint_key: "Quitar la copia del archivo (Ctrl+Z)",
    undo_move_hint_key: "Devolver el archivo a su sitio anterior (Ctrl+Z)",
    undo: "Deshacer",
    last_filed: |path| format!("Último: {path}"),

    files_button: "Archivos",
    files_hint: "Añadir uno o varios archivos a la cola",
    folder_button: "Carpeta",
    folder_hint: "Añadir todos los documentos de una carpeta a la cola",
    scanning: "Escaneando …",
    preparing: "Procesando …",
    scan: "Escanear",
    scan_hint: "Traer del escáner – varias páginas forman un solo PDF",
    scan_busy: "Ya se está escaneando",
    scan_as: "Escanear como",
    queue_counts: |open, queued| format!("{open} por revisar · {queued} en la cola"),
    resume: "Reanudar",
    pause: "Pausa",
    pause_hint: "El documento en curso se termina igualmente",
    queue_empty: "No hay documentos en la cola",
    hide_done: "Ocultar archivados",
    hide_done_hint: "El último archivado se podrá seguir deshaciendo",
    nothing_waiting: "Ningún documento en espera",
    nothing_filed: "Todavía no se ha archivado nada",
    remove_waiting: "Vaciar la cola",
    paused_note: "En pausa – el documento en curso se termina igualmente.",
    row_waiting: "en espera",
    row_next: "a continuación",
    row_check: |why, folder| format!("por revisar: {why} · → {folder}"),
    row_filed: |name| format!("archivado: {name}"),
    earlier: "Procesar antes",
    later: "Procesar después",
    reanalyze: "Volver a analizar",
    remove_hint: "Quitar de la lista (el archivo se queda donde está)",

    drop_here: "Arrastra documentos aquí",
    supported: "Escaneos y PDF, fotos (JPG, PNG), Word, Excel, PowerPoint y LibreOffice, Markdown y texto – de uno en \
                uno o una carpeta entera. Los demás archivos (p. ej., ZIP) se pueden archivar por su nombre de \
                archivo.",
    steps: [
        "Scrinium lee cada documento en local y propone fecha, tipo, remitente, carpeta y nombre.",
        "Revisas la propuesta a la derecha y cambias lo que no encaje.",
        "«Archivar» (Ctrl+Intro) mueve el archivo; «Deshacer» lo devuelve a su sitio.",
    ],
    choose_files: "Elegir archivos …",
    choose_folder: "Elegir carpeta …",
    scan_dots: "Escanear …",
    archive_absent: |root| format!("La carpeta del archivo {root} aún no existe."),
    choose_archive: "Elegir carpeta del archivo …",
    create_here: "Crear aquí",
    create_failed: |e| format!("No se pudo crear la carpeta: {e}"),
    archive_summary: |root, folders, offered, rules| {
        let folders = match folders {
            1 => "1 carpeta".to_string(),
            n => format!("{n} carpetas"),
        };
        let rules = match rules {
            1 => "1 regla".to_string(),
            n => format!("{n} reglas"),
        };
        format!("Archivo {root} · {folders}, {offered} para la IA · {rules}")
    },
    no_model_yet: "Todavía no hay ningún modelo de IA cargado – mientras tanto ayudan el reconocimiento de texto, las \
                   palabras clave y las reglas.",
    server_unreachable: "No se puede conectar con el servidor de IA – mientras tanto ayudan el reconocimiento de \
                         texto, las palabras clave y las reglas.",
    set_up_ai: "Configurar la IA",
    ai_disabled: "La IA está desactivada – análisis mediante reconocimiento de texto, palabras clave y reglas.",
    new_here: "¿Nuevo aquí? Describe tu archivo y configura con IA …",

    select_document: "Selecciona un documento a la izquierda",
    from_dir: |dir| format!("de {dir}"),
    waiting_slot: |i, n| format!("En espera en la cola ({i} de {n})"),
    waiting_in_queue: "En espera en la cola",
    analysed_next: "Se analizará a continuación …",
    queue_paused: "La cola está en pausa.",
    one_by_one: "Los documentos se procesan uno tras otro.",
    working_slot: |i, n, secs| {
        let time = secs.map(|s| format!(" · desde hace {s} s")).unwrap_or_default();
        format!("Documento {i} de {n} · todo se ejecuta en local{time}")
    },
    analysis_failed: "Error en el análisis",
    try_again: "Reintentar",
    file_anyway: "Archivar de todos modos …",
    file_anyway_hint: "Escribe tú el nombre y la carpeta",
    filed_as_copy: "Archivado como copia",
    filed: "Archivado",
    show_in_file_manager: "Mostrar en el explorador",
    undo_move_hint: "Devolver el archivo a su sitio anterior",

    no_matching_folder: "ninguna carpeta adecuada",
    uncertain: "dudoso",
    archive_unreachable: "archivo inaccesible",
    new_folder: "carpeta nueva",
    note: "aviso",

    remove_from_list: "Quitar de la lista",
    discard_title: "¿Descartar los cambios?",
    discard_text: |what| format!("Has cambiado la propuesta. «{what}» descarta estos cambios."),
    back: "Volver",

    archive: "Archivo",
    drop_release: "Suelta para analizar",
    notice_close: "Haz clic para cerrar",
};

pub static IT: Texts = Texts {
    defaults_active: |e| format!("{e} – valori predefiniti attivi."),

    queued: |n, name| match n {
        1 => format!("«{name}» in coda"),
        n => format!("{n} documenti in coda"),
    },
    skipped: |n| match n {
        1 => "1 saltato (file di sistema o non leggibile)".to_string(),
        n => format!("{n} saltati (file di sistema o non leggibili)"),
    },
    already_listed: |n| format!("{n} già nell'elenco"),
    subfolders_left_out: |n| match n {
        1 => "1 sottocartella non inclusa".to_string(),
        n => format!("{n} sottocartelle non incluse"),
    },
    worker_stopped: "Il processo di analisi si è arrestato",
    setup_ready: "La proposta di impostazioni è pronta",

    tags_written: " · Tag salvati per la ricerca di Windows",
    tags_skipped: |why| format!(" · Tag non scritti ({why})"),
    tags_failed: |e| format!(" · Tag non scritti: {e}"),
    journal_failed: |e| format!(" · Decisione non registrata: {e}"),
    pages_turned: " · pagine salvate ruotate",
    turn_unsupported: " · rotazione solo nell'anteprima (Scrinium non ruota questo formato)",
    turn_skipped: |why| format!(" · pagine non ruotate ({why})"),
    turn_failed: |e| format!(" · pagine non ruotate: {e}"),
    filed_toast: |path| format!("Archiviato: {path}"),
    file_failed: "Archiviazione non riuscita – il motivo è nel modulo",
    undo_copy_removed: |name| format!("Annullato: {name} – la copia nell'archivio è stata rimossa"),
    undo_moved_back: |name| format!("Annullato: {name} – è di nuovo nella posizione precedente"),
    undo_failed: |e| format!("Impossibile annullare: {e}"),
    undo_last_copy_removed: |name| format!("Annullato: copia di {name} rimossa"),
    undo_last_moved_back: |name| format!("Annullato: {name} è di nuovo nella posizione precedente e nell'elenco"),
    analysis_failed_warning: |e| format!("Analisi non riuscita: {e}"),

    scanner_opening: "Apertura dello scanner …",
    scan_cancelled: "La scansione è stata annullata.",
    scan_error: |e| format!("Scansione: {e}"),
    scan_dir_failed: |dir, e| format!("Scansione non salvata: impossibile creare {dir} ({e})"),
    scan_preparing: "Elaborazione della scansione …",
    preparation_cancelled: "L'elaborazione è stata interrotta.",
    scan_saved: |path| format!("Scansione salvata: {path}"),
    scan_not_saved: |e| format!("Scansione non salvata: {e}"),

    setup_waiting: "In attesa dell'IA …",
    config_reloaded: "Configurazione ricaricata",
    settings_saved: "Impostazioni salvate",

    models_available: |url, n| match n {
        1 => format!("{url} · 1 modello disponibile"),
        n => format!("{url} · {n} modelli disponibili"),
    },
    no_model_loaded: "Nessun modello caricato",

    pick_files_title: "Scegli documenti",
    documents: "Documenti",
    all_files: "Tutti i file",
    pick_folder_title: "Cartella con documenti",

    ai_status_checking: "controllo in corso …",
    ai_status_off: "disattivata",

    settings: "Impostazioni",
    settings_newer: |v| format!("Impostazioni – Scrinium {v} è disponibile"),

    ai_download: |p| format!("Download IA {p:.0}%"),
    ai_off: "IA disattivata",
    ai_off_hint: "Solo riconoscimento del testo, parole chiave e regole",
    ai_checking: "Controllo dell'IA …",
    ai_model: |m| format!("IA: {m}"),
    ai_no_model: "IA: nessun modello",
    ai_unreachable: "IA non raggiungibile",
    ai_meanwhile: |e| format!("{e}\nNel frattempo: riconoscimento del testo, parole chiave e regole."),
    ai_click_settings: "Fai clic per aprire le impostazioni dell'IA",
    ai_click_check: "Fai clic per controllare di nuovo",

    back_to_documents: "Torna ai documenti",
    show_list: "Mostra elenco",
    hide_list: "Nascondi elenco – più spazio per documento e modulo",
    status_counts: |open, running, waiting, paused, filed| {
        let pause = if paused { " · in pausa" } else { "" };
        let filed = match filed {
            1 => "1 archiviato".to_string(),
            n => format!("{n} archiviati"),
        };
        format!("{open} da controllare · {running} in corso · {waiting} in attesa{pause} · {filed}")
    },
    archive_status: |root, n| match n {
        1 => format!("Archivio: {root} · 1 cartella"),
        n => format!("Archivio: {root} · {n} cartelle"),
    },
    archive_missing: |root| format!("Archivio mancante: {root}"),
    click_opens_folder: |path| format!("{path}\nFai clic per aprire la cartella"),
    undo_copy_hint_key: "Rimuovi la copia dall'archivio (Ctrl+Z)",
    undo_move_hint_key: "Riporta il file nella posizione precedente (Ctrl+Z)",
    undo: "Annulla",
    last_filed: |path| format!("Ultimo: {path}"),

    files_button: "File",
    files_hint: "Aggiungi uno o più file alla coda",
    folder_button: "Cartella",
    folder_hint: "Aggiungi tutti i documenti di una cartella alla coda",
    scanning: "Scansione …",
    preparing: "Elaborazione …",
    scan: "Scansiona",
    scan_hint: "Acquisisci dallo scanner – più pagine diventano un solo PDF",
    scan_busy: "È già in corso una scansione",
    scan_as: "Scansiona come",
    queue_counts: |open, queued| format!("{open} da controllare · {queued} in coda"),
    resume: "Riprendi",
    pause: "Pausa",
    pause_hint: "Il documento in corso viene comunque completato",
    queue_empty: "Nessun documento in coda",
    hide_done: "Nascondi archiviati",
    hide_done_hint: "L'ultima archiviazione si potrà ancora annullare",
    nothing_waiting: "Nessun documento in attesa",
    nothing_filed: "Non è ancora stato archiviato nulla",
    remove_waiting: "Svuota la coda",
    paused_note: "In pausa – il documento in corso viene comunque completato.",
    row_waiting: "in attesa",
    row_next: "a seguire",
    row_check: |why, folder| format!("da controllare: {why} · → {folder}"),
    row_filed: |name| format!("archiviato: {name}"),
    earlier: "Elabora prima",
    later: "Elabora dopo",
    reanalyze: "Rianalizza",
    remove_hint: "Togli dall'elenco (il file resta dov'è)",

    drop_here: "Trascina qui i documenti",
    supported: "Scansioni e PDF, foto (JPG, PNG), Word, Excel, PowerPoint e LibreOffice, Markdown e testo – uno alla \
                volta o un'intera cartella. Gli altri file (ad es. ZIP) si possono archiviare in base al nome del \
                file.",
    steps: [
        "Scrinium legge ogni documento in locale e propone data, tipo, mittente, cartella e nome.",
        "Controlli la proposta a destra e cambi ciò che non va.",
        "«Archivia» (Ctrl+Invio) sposta il file; «Annulla» lo riporta indietro.",
    ],
    choose_files: "Scegli file …",
    choose_folder: "Scegli cartella …",
    scan_dots: "Scansiona …",
    archive_absent: |root| format!("La cartella dell'archivio {root} non esiste ancora."),
    choose_archive: "Scegli cartella dell'archivio …",
    create_here: "Crea qui",
    create_failed: |e| format!("Impossibile creare la cartella: {e}"),
    archive_summary: |root, folders, offered, rules| {
        let folders = match folders {
            1 => "1 cartella".to_string(),
            n => format!("{n} cartelle"),
        };
        let rules = match rules {
            1 => "1 regola".to_string(),
            n => format!("{n} regole"),
        };
        format!("Archivio {root} · {folders}, di cui {offered} per l'IA · {rules}")
    },
    no_model_yet: "Nessun modello di IA ancora caricato – nel frattempo aiutano il riconoscimento del testo, le parole \
                   chiave e le regole.",
    server_unreachable: "Il server IA non è raggiungibile – nel frattempo aiutano il riconoscimento del testo, le \
                         parole chiave e le regole.",
    set_up_ai: "Configura l'IA",
    ai_disabled: "L'IA è disattivata – analisi tramite riconoscimento del testo, parole chiave e regole.",
    new_here: "Nuovo qui? Descrivi il tuo archivio e configura con l'IA …",

    select_document: "Seleziona un documento a sinistra",
    from_dir: |dir| format!("da {dir}"),
    waiting_slot: |i, n| format!("In attesa nella coda ({i} di {n})"),
    waiting_in_queue: "In attesa nella coda",
    analysed_next: "Sarà analizzato a seguire …",
    queue_paused: "La coda è in pausa.",
    one_by_one: "I documenti vengono elaborati uno dopo l'altro.",
    working_slot: |i, n, secs| {
        let time = secs.map(|s| format!(" · da {s} s")).unwrap_or_default();
        format!("Documento {i} di {n} · tutto in locale{time}")
    },
    analysis_failed: "Analisi non riuscita",
    try_again: "Riprova",
    file_anyway: "Archivia comunque …",
    file_anyway_hint: "Inserisci tu nome e cartella",
    filed_as_copy: "Archiviato come copia",
    filed: "Archiviato",
    show_in_file_manager: "Mostra nel gestore file",
    undo_move_hint: "Riporta il file nella posizione precedente",

    no_matching_folder: "nessuna cartella adatta",
    uncertain: "incerto",
    archive_unreachable: "archivio non raggiungibile",
    new_folder: "nuova cartella",
    note: "avviso",

    remove_from_list: "Togli dall'elenco",
    discard_title: "Scartare le modifiche?",
    discard_text: |what| format!("Hai modificato la proposta. «{what}» scarta queste modifiche."),
    back: "Indietro",

    archive: "Archivio",
    drop_release: "Rilascia per analizzare",
    notice_close: "Fai clic per chiudere",
};
