//! Analysis (`analysis.rs`, `ocr.rs`, `preprocess.rs`): progress, hints, origin of the fields, image correction.
//!
//! Not here: the instructions to the AI including folder notes, and everything the AI answers.

pub struct Texts {
    // Origin of a value in the suggestion
    pub origin_ai: &'static str,
    pub origin_rule: &'static str,
    pub origin_keyword: &'static str,
    pub origin_text: &'static str,
    pub origin_filename: &'static str,
    pub origin_folder: &'static str,
    pub origin_fallback: &'static str,

    // AI: model and requests about the document
    pub loading_ai_model: &'static str,
    pub ai_model_not_loaded: &'static str,
    pub ai_without_page_image: fn(&str) -> String,
    pub failed_with_image_too: fn(&str) -> String,
    pub ai_analyses_on: fn(&str) -> String,
    pub ai_analyses: &'static str,
    /// Input tokens, seconds, output tokens, seconds.
    pub ai_stats: fn(usize, f32, usize, f32) -> String,
    pub ai_reads_document_on: fn(&str) -> String,
    pub ai_reads_document: &'static str,
    pub ai_folder_choice_failed: fn(&str) -> String,
    pub ai_determines_fields: &'static str,
    /// Seconds for description, folder choice and fields.
    pub step_times: fn(f32, f32, f32) -> String,

    // Folder choice level by level
    pub ai_chooses_folder: fn(usize) -> String,
    pub no_subfolder_fits: fn(&str) -> String,
    /// Folder, reason.
    pub folder_reason: fn(&str, &str) -> String,

    // Settings from a description
    pub ai_switched_off: &'static str,
    pub ai_reads_description: fn(&str) -> String,
    pub ai_writes_settings: fn(usize) -> String,
    /// Characters in the description, maximum allowed characters.
    pub description_too_long: fn(usize, usize) -> String,
    pub ai_server_writes_settings: &'static str,
    pub unexpected_answer: fn(&str) -> String,

    // Loading, orientation, text recognition
    pub starting_ocr: &'static str,
    pub loading_document: &'static str,
    pub ocr_unavailable: fn(&str) -> String,
    pub pdf_text_layer: &'static str,
    /// Name of the OCR engine; the text comes partly from the text layer, partly from OCR (PDF with text and image
    /// pages).
    pub text_layer_and_ocr: fn(&str) -> String,
    /// Page: its text layer is garbled and was not used.
    pub garbled_text_layer: fn(&str) -> String,
    /// Page, page count.
    pub page: fn(usize, usize) -> String,
    pub detecting_orientation: fn(&str) -> String,
    pub recognising_text: fn(&str) -> String,
    /// Page, error.
    pub ocr_failed: fn(&str, &str) -> String,
    pub no_readable_content: &'static str,
    pub no_text: &'static str,
    pub legacy_office: &'static str,
    pub ai_not_used: fn(&str) -> String,

    // Image correction
    pub rotated: fn(u16) -> String,
    pub straightened: fn(f32) -> String,
    /// Page, corrections.
    pub correction: fn(usize, &str) -> String,

    // Reasons and notes on the folder
    pub keywords_found: fn(&[String]) -> String,
    pub unknown_folder_replaced: fn(&str) -> String,
    pub year_folder: fn(i32) -> String,
    pub in_decade_folder: &'static str,
    pub nothing_found: &'static str,
    pub no_year_folder: fn(i32) -> String,
    /// Rule name, its folder.
    pub rule_applied: fn(&str, &str) -> String,

    // Text recognition with Tesseract (ocr.rs, tesseract.rs)
    /// Version, languages: "Tesseract 5.5.3 (deu+eng)".
    pub ocr_name: fn(&str, &str) -> String,
    #[cfg_attr(any(not(windows), tesseract_embedded), allow(dead_code))]
    pub tesseract_not_embedded: &'static str,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub tesseract_missing_linux: &'static str,
    #[cfg_attr(any(windows, target_os = "linux"), allow(dead_code))]
    pub tesseract_unsupported: &'static str,
    pub tesseract_load_failed: fn(&str) -> String,
    pub tesseract_init_failed: fn(&str) -> String,
    pub tesseract_languages_missing: fn(&str) -> String,
    pub tesseract_no_text: &'static str,
    pub tessdata_path_unusable: fn(&str) -> String,
    /// Language, size.
    pub loading_language: fn(&str, &str) -> String,
    pub language_corrupt: fn(&str) -> String,
    pub language_unknown: fn(&str) -> String,
    pub no_languages: &'static str,
}

pub fn t() -> &'static Texts {
    super::pick(&DE, &EN, &FR, &ES, &IT)
}

/// Decimal comma instead of point (German, French, Spanish, Italian).
fn comma(s: String) -> String {
    s.replace('.', ",")
}

pub static DE: Texts = Texts {
    origin_ai: "KI",
    origin_rule: "Regel",
    origin_keyword: "Stichwort",
    origin_text: "aus dem Text",
    origin_filename: "aus dem Dateinamen",
    origin_folder: "aus dem Ordner",
    origin_fallback: "Auffangordner",

    loading_ai_model: "Lade KI-Modell …",
    ai_model_not_loaded: "KI-Modell ist nicht geladen",
    ai_without_page_image: |e| format!("KI ohne Seitenbild: {e}"),
    failed_with_image_too: |e| format!("auch mit Bild fehlgeschlagen: {e}"),
    ai_analyses_on: |device| format!("KI analysiert das Dokument ({device}) …"),
    ai_analyses: "KI analysiert das Dokument …",
    ai_stats: |input, input_s, output, output_s| {
        comma(format!("Eingabe {input} Token in {input_s:.1} s, Antwort {output} Token in {output_s:.1} s"))
    },
    ai_reads_document_on: |device| format!("KI liest das Dokument ({device}) …"),
    ai_reads_document: "KI liest das Dokument …",
    ai_folder_choice_failed: |e| format!("Ordnerwahl der KI: {e}"),
    ai_determines_fields: "KI bestimmt die Angaben für den Dateinamen …",
    step_times: |a, b, c| comma(format!("Beschreibung {a:.1} s, Ordnerwahl {b:.1} s, Felder {c:.1} s")),

    ai_chooses_folder: |level| format!("KI wählt den Ordner (Ebene {level}) …"),
    no_subfolder_fits: |reason| format!("kein Unterordner passt: {reason}"),
    folder_reason: |folder, reason| format!("{folder}: {reason}"),

    ai_switched_off: "Die KI ist ausgeschaltet (Einstellungen, Reiter KI)",
    ai_reads_description: |device| format!("KI liest die Beschreibung ({device}) …"),
    ai_writes_settings: |chars| format!("KI schreibt die Einstellungen … bisher etwa {chars} Zeichen"),
    description_too_long: |chars, max| {
        format!("Die Beschreibung ist zu lang (etwa {chars} Zeichen, höchstens etwa {max}) – bitte kürzen")
    },
    ai_server_writes_settings: "KI-Server schreibt die Einstellungen …",
    unexpected_answer: |e| format!("Antwort der KI hat ein unerwartetes Format: {e}"),

    starting_ocr: "Starte Texterkennung …",
    loading_document: "Lade Dokument …",
    ocr_unavailable: |e| format!("Texterkennung nicht verfügbar: {e}"),
    pdf_text_layer: "PDF-Textebene",
    text_layer_and_ocr: |ocr| format!("PDF-Textebene und {ocr}"),
    garbled_text_layer: |page| format!("{page}: Die Textebene ist unlesbar und wurde nicht verwendet."),
    page: |page, total| format!("Seite {page}/{total}"),
    detecting_orientation: |page| format!("{page}: Ausrichtung erkennen …"),
    recognising_text: |page| format!("{page}: Texterkennung …"),
    ocr_failed: |page, e| format!("{page}: OCR fehlgeschlagen: {e}"),
    no_readable_content: "Kein lesbarer Inhalt – der Vorschlag stützt sich auf den Dateinamen.",
    no_text: "Kein Text erkannt.",
    legacy_office: "Altes Office-Format: Text nur näherungsweise gelesen.",
    ai_not_used: |e| format!("KI nicht verwendet: {e}"),

    rotated: |degrees| format!("{degrees}° gedreht"),
    straightened: |degrees| comma(format!("{degrees:.1}° begradigt")),
    correction: |page, parts| format!("Seite {page}: {parts}"),

    keywords_found: |words| format!("Stichwort „{}“", words.join("“, „")),
    unknown_folder_replaced: |folder| format!("KI schlug den nicht vorhandenen Ordner „{folder}“ vor – ersetzt."),
    year_folder: |year| format!("Jahresordner für {year}"),
    in_decade_folder: "im Jahrzehnt-Ordner",
    nothing_found: "nichts Passendes gefunden",
    no_year_folder: |year| format!("kein Jahresordner für {year}"),
    rule_applied: |rule, folder| format!("Regel „{rule}“: Ordner {folder}"),

    ocr_name: |v, l| format!("Tesseract {v} ({l})"),
    tesseract_not_embedded: "Diese Scrinium-Version enthält kein Tesseract (eigener Build ohne die Tesseract-DLL).",
    tesseract_missing_linux: "Tesseract fehlt – unter Ubuntu und Debian: sudo apt install libtesseract5 tesseract-ocr-deu tesseract-ocr-eng",
    tesseract_unsupported: "Texterkennung gibt es auf diesem System nicht.",
    tesseract_load_failed: |e| format!("Tesseract ließ sich nicht laden: {e}"),
    tesseract_init_failed: |l| format!("Tesseract startet nicht mit den Sprachen {l}"),
    tesseract_languages_missing: |l| format!("Sprachdaten nicht geladen: {l}"),
    tesseract_no_text: "Tesseract lieferte keinen Text",
    tessdata_path_unusable: |p| format!("Den Ordner für die Sprachdaten kann Tesseract nicht verwenden: {p}"),
    loading_language: |l, s| format!("Lade Sprachdaten {l} (einmalig, {s}) …"),
    language_corrupt: |l| format!("Sprachdaten {l} sind beschädigt – beim nächsten Mal werden sie neu geladen"),
    language_unknown: |l| format!("Für „{l}“ gibt es keine Tesseract-Sprachdaten"),
    no_languages: "Keine Sprache für die Texterkennung gewählt",
};

pub static EN: Texts = Texts {
    origin_ai: "AI",
    origin_rule: "rule",
    origin_keyword: "keyword",
    origin_text: "from the text",
    origin_filename: "from the file name",
    origin_folder: "from the folder",
    origin_fallback: "fallback folder",

    loading_ai_model: "Loading AI model …",
    ai_model_not_loaded: "AI model is not loaded",
    ai_without_page_image: |e| format!("AI without the page image: {e}"),
    failed_with_image_too: |e| format!("also failed with the image: {e}"),
    ai_analyses_on: |device| format!("AI is analysing the document ({device}) …"),
    ai_analyses: "AI is analysing the document …",
    ai_stats: |input, input_s, output, output_s| {
        let tokens = |n: usize| if n == 1 { "token" } else { "tokens" };
        format!(
            "Input {input} {} in {input_s:.1} s, answer {output} {} in {output_s:.1} s",
            tokens(input),
            tokens(output)
        )
    },
    ai_reads_document_on: |device| format!("AI is reading the document ({device}) …"),
    ai_reads_document: "AI is reading the document …",
    ai_folder_choice_failed: |e| format!("AI folder choice: {e}"),
    ai_determines_fields: "AI is working out the details for the file name …",
    step_times: |a, b, c| format!("Description {a:.1} s, folder choice {b:.1} s, fields {c:.1} s"),

    ai_chooses_folder: |level| format!("AI is choosing the folder (level {level}) …"),
    no_subfolder_fits: |reason| format!("no subfolder fits: {reason}"),
    folder_reason: |folder, reason| format!("{folder}: {reason}"),

    ai_switched_off: "AI is switched off (Settings, AI tab)",
    ai_reads_description: |device| format!("AI is reading the description ({device}) …"),
    ai_writes_settings: |chars| {
        format!(
            "AI is writing the settings … about {chars} {} so far",
            if chars == 1 { "character" } else { "characters" }
        )
    },
    description_too_long: |chars, max| {
        format!("The description is too long (about {chars} characters, at most about {max}) – please shorten it")
    },
    ai_server_writes_settings: "AI server is writing the settings …",
    unexpected_answer: |e| format!("The AI's answer has an unexpected format: {e}"),

    starting_ocr: "Starting text recognition …",
    loading_document: "Loading document …",
    ocr_unavailable: |e| format!("Text recognition not available: {e}"),
    pdf_text_layer: "PDF text layer",
    text_layer_and_ocr: |ocr| format!("PDF text layer and {ocr}"),
    garbled_text_layer: |page| format!("{page}: the text layer is unreadable and was not used."),
    page: |page, total| format!("Page {page}/{total}"),
    detecting_orientation: |page| format!("{page}: detecting orientation …"),
    recognising_text: |page| format!("{page}: recognising text …"),
    ocr_failed: |page, e| format!("{page}: OCR failed: {e}"),
    no_readable_content: "No readable content – the suggestion is based on the file name.",
    no_text: "No text recognised.",
    legacy_office: "Old Office format: text read only approximately.",
    ai_not_used: |e| format!("AI not used: {e}"),

    rotated: |degrees| format!("rotated {degrees}°"),
    straightened: |degrees| format!("straightened {degrees:.1}°"),
    correction: |page, parts| format!("Page {page}: {parts}"),

    keywords_found: |words| {
        format!("{} “{}”", if words.len() == 1 { "keyword" } else { "keywords" }, words.join("”, “"))
    },
    unknown_folder_replaced: |folder| format!("AI suggested the folder “{folder}”, which does not exist – replaced."),
    year_folder: |year| format!("year folder for {year}"),
    in_decade_folder: "in the decade folder",
    nothing_found: "nothing suitable found",
    no_year_folder: |year| format!("no year folder for {year}"),
    rule_applied: |rule, folder| format!("Rule “{rule}”: folder {folder}"),

    ocr_name: |v, l| format!("Tesseract {v} ({l})"),
    tesseract_not_embedded: "This Scrinium build does not include Tesseract (own build without the Tesseract DLL).",
    tesseract_missing_linux: "Tesseract is missing – on Ubuntu and Debian: sudo apt install libtesseract5 tesseract-ocr-deu tesseract-ocr-eng",
    tesseract_unsupported: "Text recognition is not available on this system.",
    tesseract_load_failed: |e| format!("Tesseract could not be loaded: {e}"),
    tesseract_init_failed: |l| format!("Tesseract does not start with the languages {l}"),
    tesseract_languages_missing: |l| format!("Language data not loaded: {l}"),
    tesseract_no_text: "Tesseract returned no text",
    tessdata_path_unusable: |p| format!("Tesseract cannot use the folder for the language data: {p}"),
    loading_language: |l, s| format!("Downloading language data {l} (once only, {s}) …"),
    language_corrupt: |l| format!("Language data {l} is damaged – it will be downloaded again next time"),
    language_unknown: |l| format!("There is no Tesseract language data for “{l}”"),
    no_languages: "No language selected for text recognition",
};

pub static FR: Texts = Texts {
    origin_ai: "IA",
    origin_rule: "règle",
    origin_keyword: "mot-clé",
    origin_text: "d'après le texte",
    origin_filename: "d'après le nom de fichier",
    origin_folder: "d'après le dossier",
    origin_fallback: "dossier de repli",

    loading_ai_model: "Chargement du modèle d'IA …",
    ai_model_not_loaded: "Le modèle d'IA n'est pas chargé",
    ai_without_page_image: |e| format!("IA sans l'image de la page : {e}"),
    failed_with_image_too: |e| format!("échec également avec l'image : {e}"),
    ai_analyses_on: |device| format!("L'IA analyse le document ({device}) …"),
    ai_analyses: "L'IA analyse le document …",
    ai_stats: |input, input_s, output, output_s| {
        let tokens = |n: usize| if n < 2 { "token" } else { "tokens" };
        comma(format!(
            "Entrée {input} {} en {input_s:.1} s, réponse {output} {} en {output_s:.1} s",
            tokens(input),
            tokens(output)
        ))
    },
    ai_reads_document_on: |device| format!("L'IA lit le document ({device}) …"),
    ai_reads_document: "L'IA lit le document …",
    ai_folder_choice_failed: |e| format!("Choix du dossier par l'IA : {e}"),
    ai_determines_fields: "L'IA détermine les informations du nom de fichier …",
    step_times: |a, b, c| comma(format!("Description {a:.1} s, choix du dossier {b:.1} s, champs {c:.1} s")),

    ai_chooses_folder: |level| format!("L'IA choisit le dossier (niveau {level}) …"),
    no_subfolder_fits: |reason| format!("aucun sous-dossier ne convient : {reason}"),
    folder_reason: |folder, reason| format!("{folder} : {reason}"),

    ai_switched_off: "L'IA est désactivée (Paramètres, onglet IA)",
    ai_reads_description: |device| format!("L'IA lit la description ({device}) …"),
    ai_writes_settings: |chars| {
        format!(
            "L'IA rédige les paramètres … environ {chars} {} jusqu'ici",
            if chars < 2 { "caractère" } else { "caractères" }
        )
    },
    description_too_long: |chars, max| {
        format!(
            "La description est trop longue (environ {chars} caractères, au maximum environ {max}) – veuillez la \
             raccourcir"
        )
    },
    ai_server_writes_settings: "Le serveur d'IA rédige les paramètres …",
    unexpected_answer: |e| format!("La réponse de l'IA a un format inattendu : {e}"),

    starting_ocr: "Démarrage de la reconnaissance de texte …",
    loading_document: "Chargement du document …",
    ocr_unavailable: |e| format!("Reconnaissance de texte indisponible : {e}"),
    pdf_text_layer: "Couche texte du PDF",
    text_layer_and_ocr: |ocr| format!("Couche texte du PDF et {ocr}"),
    garbled_text_layer: |page| format!("{page} : la couche texte est illisible et n'a pas été utilisée."),
    page: |page, total| format!("Page {page}/{total}"),
    detecting_orientation: |page| format!("{page} : détection de l'orientation …"),
    recognising_text: |page| format!("{page} : reconnaissance de texte …"),
    ocr_failed: |page, e| format!("{page} : échec de l'OCR : {e}"),
    no_readable_content: "Aucun contenu lisible – la proposition s'appuie sur le nom de fichier.",
    no_text: "Aucun texte reconnu.",
    legacy_office: "Ancien format Office : texte lu de façon approximative.",
    ai_not_used: |e| format!("IA non utilisée : {e}"),

    rotated: |degrees| format!("pivotée de {degrees}°"),
    straightened: |degrees| comma(format!("redressée de {degrees:.1}°")),
    correction: |page, parts| format!("Page {page} : {parts}"),

    keywords_found: |words| {
        format!("{} « {} »", if words.len() == 1 { "mot-clé" } else { "mots-clés" }, words.join(" », « "))
    },
    unknown_folder_replaced: |folder| format!("L'IA a proposé le dossier inexistant « {folder} » – remplacé."),
    year_folder: |year| format!("dossier annuel pour {year}"),
    in_decade_folder: "dans le dossier de la décennie",
    nothing_found: "rien de pertinent trouvé",
    no_year_folder: |year| format!("pas de dossier annuel pour {year}"),
    rule_applied: |rule, folder| format!("Règle « {rule} » : dossier {folder}"),

    ocr_name: |v, l| format!("Tesseract {v} ({l})"),
    tesseract_not_embedded: "Cette version de Scrinium ne contient pas Tesseract (compilation sans la DLL Tesseract).",
    tesseract_missing_linux: "Tesseract est absent – sous Ubuntu et Debian : sudo apt install libtesseract5 tesseract-ocr-deu tesseract-ocr-eng",
    tesseract_unsupported: "La reconnaissance de texte n'est pas disponible sur ce système.",
    tesseract_load_failed: |e| format!("Impossible de charger Tesseract : {e}"),
    tesseract_init_failed: |l| format!("Tesseract ne démarre pas avec les langues {l}"),
    tesseract_languages_missing: |l| format!("Données de langue non chargées : {l}"),
    tesseract_no_text: "Tesseract n'a renvoyé aucun texte",
    tessdata_path_unusable: |p| format!("Tesseract ne peut pas utiliser le dossier des données de langue : {p}"),
    loading_language: |l, s| format!("Téléchargement des données de langue {l} (une seule fois, {s}) …"),
    language_corrupt: |l| {
        format!("Les données de langue {l} sont endommagées – elles seront retéléchargées la prochaine fois")
    },
    language_unknown: |l| format!("Il n'existe pas de données de langue Tesseract pour « {l} »"),
    no_languages: "Aucune langue choisie pour la reconnaissance de texte",
};

pub static ES: Texts = Texts {
    origin_ai: "IA",
    origin_rule: "regla",
    origin_keyword: "palabra clave",
    origin_text: "del texto",
    origin_filename: "del nombre de archivo",
    origin_folder: "de la carpeta",
    origin_fallback: "carpeta de reserva",

    loading_ai_model: "Cargando el modelo de IA …",
    ai_model_not_loaded: "El modelo de IA no está cargado",
    ai_without_page_image: |e| format!("IA sin la imagen de la página: {e}"),
    failed_with_image_too: |e| format!("también falló con la imagen: {e}"),
    ai_analyses_on: |device| format!("La IA analiza el documento ({device}) …"),
    ai_analyses: "La IA analiza el documento …",
    ai_stats: |input, input_s, output, output_s| {
        let tokens = |n: usize| if n == 1 { "token" } else { "tokens" };
        comma(format!(
            "Entrada {input} {} en {input_s:.1} s, respuesta {output} {} en {output_s:.1} s",
            tokens(input),
            tokens(output)
        ))
    },
    ai_reads_document_on: |device| format!("La IA lee el documento ({device}) …"),
    ai_reads_document: "La IA lee el documento …",
    ai_folder_choice_failed: |e| format!("Elección de carpeta de la IA: {e}"),
    ai_determines_fields: "La IA determina los datos para el nombre de archivo …",
    step_times: |a, b, c| comma(format!("Descripción {a:.1} s, elección de carpeta {b:.1} s, campos {c:.1} s")),

    ai_chooses_folder: |level| format!("La IA elige la carpeta (nivel {level}) …"),
    no_subfolder_fits: |reason| format!("ninguna subcarpeta encaja: {reason}"),
    folder_reason: |folder, reason| format!("{folder}: {reason}"),

    ai_switched_off: "La IA está desactivada (Ajustes, pestaña IA)",
    ai_reads_description: |device| format!("La IA lee la descripción ({device}) …"),
    ai_writes_settings: |chars| {
        format!(
            "La IA escribe los ajustes … hasta ahora unos {chars} {}",
            if chars == 1 { "carácter" } else { "caracteres" }
        )
    },
    description_too_long: |chars, max| {
        format!("La descripción es demasiado larga (unos {chars} caracteres, como máximo unos {max}) – acórtala")
    },
    ai_server_writes_settings: "El servidor de IA escribe los ajustes …",
    unexpected_answer: |e| format!("La respuesta de la IA tiene un formato inesperado: {e}"),

    starting_ocr: "Iniciando el reconocimiento de texto …",
    loading_document: "Cargando el documento …",
    ocr_unavailable: |e| format!("Reconocimiento de texto no disponible: {e}"),
    pdf_text_layer: "Capa de texto del PDF",
    text_layer_and_ocr: |ocr| format!("Capa de texto del PDF y {ocr}"),
    garbled_text_layer: |page| format!("{page}: la capa de texto es ilegible y no se ha utilizado."),
    page: |page, total| format!("Página {page}/{total}"),
    detecting_orientation: |page| format!("{page}: detectando la orientación …"),
    recognising_text: |page| format!("{page}: reconocimiento de texto …"),
    ocr_failed: |page, e| format!("{page}: falló el OCR: {e}"),
    no_readable_content: "No hay contenido legible – la propuesta se basa en el nombre de archivo.",
    no_text: "No se ha reconocido texto.",
    legacy_office: "Formato antiguo de Office: texto leído solo de forma aproximada.",
    ai_not_used: |e| format!("IA no utilizada: {e}"),

    rotated: |degrees| format!("girada {degrees}°"),
    straightened: |degrees| comma(format!("enderezada {degrees:.1}°")),
    correction: |page, parts| format!("Página {page}: {parts}"),

    keywords_found: |words| {
        format!("{} «{}»", if words.len() == 1 { "palabra clave" } else { "palabras clave" }, words.join("», «"))
    },
    unknown_folder_replaced: |folder| format!("La IA propuso la carpeta inexistente «{folder}» – reemplazada."),
    year_folder: |year| format!("carpeta anual de {year}"),
    in_decade_folder: "en la carpeta de la década",
    nothing_found: "no se encontró nada adecuado",
    no_year_folder: |year| format!("no hay carpeta anual de {year}"),
    rule_applied: |rule, folder| format!("Regla «{rule}»: carpeta {folder}"),

    ocr_name: |v, l| format!("Tesseract {v} ({l})"),
    tesseract_not_embedded: "Esta versión de Scrinium no incluye Tesseract (compilación sin la DLL de Tesseract).",
    tesseract_missing_linux: "Falta Tesseract – en Ubuntu y Debian: sudo apt install libtesseract5 tesseract-ocr-deu tesseract-ocr-eng",
    tesseract_unsupported: "El reconocimiento de texto no está disponible en este sistema.",
    tesseract_load_failed: |e| format!("No se pudo cargar Tesseract: {e}"),
    tesseract_init_failed: |l| format!("Tesseract no arranca con los idiomas {l}"),
    tesseract_languages_missing: |l| format!("Datos de idioma no cargados: {l}"),
    tesseract_no_text: "Tesseract no devolvió texto",
    tessdata_path_unusable: |p| format!("Tesseract no puede usar la carpeta de los datos de idioma: {p}"),
    loading_language: |l, s| format!("Descargando datos de idioma {l} (una sola vez, {s}) …"),
    language_corrupt: |l| format!("Los datos de idioma {l} están dañados; se volverán a descargar la próxima vez"),
    language_unknown: |l| format!("No hay datos de idioma de Tesseract para «{l}»"),
    no_languages: "No se ha elegido ningún idioma para el reconocimiento de texto",
};

pub static IT: Texts = Texts {
    origin_ai: "IA",
    origin_rule: "regola",
    origin_keyword: "parola chiave",
    origin_text: "dal testo",
    origin_filename: "dal nome del file",
    origin_folder: "dalla cartella",
    origin_fallback: "cartella di riserva",

    loading_ai_model: "Caricamento del modello IA …",
    ai_model_not_loaded: "Il modello IA non è caricato",
    ai_without_page_image: |e| format!("IA senza l'immagine della pagina: {e}"),
    failed_with_image_too: |e| format!("non riuscito anche con l'immagine: {e}"),
    ai_analyses_on: |device| format!("L'IA analizza il documento ({device}) …"),
    ai_analyses: "L'IA analizza il documento …",
    ai_stats: |input, input_s, output, output_s| {
        comma(format!("Input {input} token in {input_s:.1} s, risposta {output} token in {output_s:.1} s"))
    },
    ai_reads_document_on: |device| format!("L'IA legge il documento ({device}) …"),
    ai_reads_document: "L'IA legge il documento …",
    ai_folder_choice_failed: |e| format!("Scelta della cartella da parte dell'IA: {e}"),
    ai_determines_fields: "L'IA determina i dati per il nome del file …",
    step_times: |a, b, c| comma(format!("Descrizione {a:.1} s, scelta della cartella {b:.1} s, campi {c:.1} s")),

    ai_chooses_folder: |level| format!("L'IA sceglie la cartella (livello {level}) …"),
    no_subfolder_fits: |reason| format!("nessuna sottocartella è adatta: {reason}"),
    folder_reason: |folder, reason| format!("{folder}: {reason}"),

    ai_switched_off: "L'IA è disattivata (Impostazioni, scheda IA)",
    ai_reads_description: |device| format!("L'IA legge la descrizione ({device}) …"),
    ai_writes_settings: |chars| {
        format!(
            "L'IA scrive le impostazioni … finora circa {chars} {}",
            if chars == 1 { "carattere" } else { "caratteri" }
        )
    },
    description_too_long: |chars, max| {
        format!("La descrizione è troppo lunga (circa {chars} caratteri, al massimo circa {max}) – accorciala")
    },
    ai_server_writes_settings: "Il server IA scrive le impostazioni …",
    unexpected_answer: |e| format!("La risposta dell'IA ha un formato inatteso: {e}"),

    starting_ocr: "Avvio del riconoscimento del testo …",
    loading_document: "Caricamento del documento …",
    ocr_unavailable: |e| format!("Riconoscimento del testo non disponibile: {e}"),
    pdf_text_layer: "Livello di testo del PDF",
    text_layer_and_ocr: |ocr| format!("Livello di testo del PDF e {ocr}"),
    garbled_text_layer: |page| format!("{page}: il livello di testo è illeggibile e non è stato usato."),
    page: |page, total| format!("Pagina {page}/{total}"),
    detecting_orientation: |page| format!("{page}: rilevamento dell'orientamento …"),
    recognising_text: |page| format!("{page}: riconoscimento del testo …"),
    ocr_failed: |page, e| format!("{page}: OCR non riuscito: {e}"),
    no_readable_content: "Nessun contenuto leggibile – la proposta si basa sul nome del file.",
    no_text: "Nessun testo riconosciuto.",
    legacy_office: "Vecchio formato Office: testo letto solo in modo approssimativo.",
    ai_not_used: |e| format!("IA non utilizzata: {e}"),

    rotated: |degrees| format!("ruotata di {degrees}°"),
    straightened: |degrees| comma(format!("raddrizzata di {degrees:.1}°")),
    correction: |page, parts| format!("Pagina {page}: {parts}"),

    keywords_found: |words| {
        format!("{} «{}»", if words.len() == 1 { "parola chiave" } else { "parole chiave" }, words.join("», «"))
    },
    unknown_folder_replaced: |folder| format!("L'IA ha proposto la cartella inesistente «{folder}» – sostituita."),
    year_folder: |year| format!("cartella annuale per il {year}"),
    in_decade_folder: "nella cartella del decennio",
    nothing_found: "non è stato trovato nulla di adatto",
    no_year_folder: |year| format!("nessuna cartella annuale per il {year}"),
    rule_applied: |rule, folder| format!("Regola «{rule}»: cartella {folder}"),

    ocr_name: |v, l| format!("Tesseract {v} ({l})"),
    tesseract_not_embedded: "Questa versione di Scrinium non contiene Tesseract (build senza la DLL di Tesseract).",
    tesseract_missing_linux: "Tesseract manca – su Ubuntu e Debian: sudo apt install libtesseract5 tesseract-ocr-deu tesseract-ocr-eng",
    tesseract_unsupported: "Il riconoscimento del testo non è disponibile su questo sistema.",
    tesseract_load_failed: |e| format!("Impossibile caricare Tesseract: {e}"),
    tesseract_init_failed: |l| format!("Tesseract non si avvia con le lingue {l}"),
    tesseract_languages_missing: |l| format!("Dati della lingua non caricati: {l}"),
    tesseract_no_text: "Tesseract non ha restituito testo",
    tessdata_path_unusable: |p| format!("Tesseract non può usare la cartella dei dati della lingua: {p}"),
    loading_language: |l, s| format!("Scaricamento dei dati della lingua {l} (una sola volta, {s}) …"),
    language_corrupt: |l| format!("I dati della lingua {l} sono danneggiati: verranno riscaricati la prossima volta"),
    language_unknown: |l| format!("Non esistono dati della lingua Tesseract per «{l}»"),
    no_languages: "Nessuna lingua scelta per il riconoscimento del testo",
};
