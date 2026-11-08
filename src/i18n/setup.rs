//! *Set up with AI* (`setup.rs`): description of the archive, suggestions to apply.

pub struct Texts {
    // Notices after applying (`setup::apply`)
    pub template_rejected: fn(&str, &str) -> String,
    pub date_format_invalid: fn(&str, &str) -> String,
    pub fallback_missing: fn(&str, &str) -> String,
    pub notes_kept: fn(usize) -> String,
    pub notes_skipped: fn(usize) -> String,
    pub rule_folder_missing: fn(&str, &str) -> String,
    pub rules_replaced: fn(usize) -> String,

    // Description of the archive
    pub intro: &'static str,
    pub load_file: &'static str,
    pub file_filter: &'static str,
    pub load_title: &'static str,
    pub insert_example: &'static str,
    pub clear: &'static str,
    /// Section not applied.
    pub section_off: &'static str,
    pub chars: fn(usize) -> String,
    pub description_hint: &'static str,

    // Generate
    pub generate: &'static str,
    /// Generate disabled: AI off.
    pub generate_needs_ai: &'static str,
    /// Generate disabled: no description.
    pub generate_needs_text: &'static str,
    /// Generate disabled: running.
    pub generate_running: &'static str,
    pub cancel_hint: &'static str,
    pub cancel: &'static str,
    pub ai_off: &'static str,
    pub ai_status: fn(&str) -> String,
    pub applied: &'static str,

    // The AI's suggestion
    pub proposal: &'static str,
    pub proposal_hint: &'static str,
    pub naming: &'static str,
    pub naming_summary: fn(&str, &str) -> String,
    pub replaces_existing: fn(usize) -> String,
    pub doctypes: fn(usize, usize) -> String,
    pub extras: fn(&str) -> String,
    pub persons: fn(usize) -> String,
    pub person_from_folder: &'static str,
    pub senders_effect: fn(usize, usize) -> String,
    pub senders: fn(usize) -> String,
    pub notes_effect: fn(usize, usize, usize) -> String,
    pub notes: fn(usize) -> String,
    pub only_empty_notes: &'static str,
    pub rules: fn(usize) -> String,
    pub rule_requires: fn(&str) -> String,
    pub instructions_effect: &'static str,
    pub instructions: &'static str,
    pub apply: &'static str,
    pub apply_hint: &'static str,
    pub discard: &'static str,
}

pub fn t() -> &'static Texts {
    super::pick(&DE, &EN, &FR, &ES, &IT)
}

/// Number with singular or plural; English, Spanish and Italian count 0 as plural.
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Like `count`, but French counts 0 and 1 as singular.
fn count_fr(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n <= 1 { one } else { many })
}

/// "3 new, 1 existing": always the first part, the others only if their number is not zero.
fn join_counts(first: String, rest: [(usize, String); 2]) -> String {
    let mut s = first;
    for (n, part) in rest {
        if n > 0 {
            s.push_str(", ");
            s.push_str(&part);
        }
    }
    s
}

pub static DE: Texts = Texts {
    template_rejected: |template, unknown| {
        format!("Vorlage „{template}“ nicht übernommen (unbekannte Bausteine {unknown})")
    },
    date_format_invalid: |bad, kept| format!("Datumsformat „{bad}“ ist ungültig – bleibt {kept}"),
    fallback_missing: |wanted, kept| format!("Auffangordner „{wanted}“ gibt es nicht – bleibt „{kept}“"),
    notes_kept: |n| format!("{n} vorhandene Ordnerbeschreibungen behalten"),
    notes_skipped: |n| format!("{n} Ordnerbeschreibungen für nicht vorhandene Ordner ausgelassen"),
    rule_folder_missing: |name, folder| format!("Regel „{name}“: Ordner „{folder}“ gibt es nicht – ausgeschaltet"),
    rules_replaced: |n| format!("{n} bisherige Sonderregeln ersetzt"),

    intro: "Beschreibe deine Ablage in eigenen Worten: Ordner, Aufbau der Dateinamen, Dokumentarten, Personen, \
            Kurznamen und feste Zuordnungen (z. B. „Kontoauszüge der Sparkasse nach Finanzen/Sparkasse/Jahr“). Eine \
            vorhandene Dokumentation lässt sich laden. Die lokale KI macht daraus Einstellungen, die du vor dem \
            Übernehmen prüfst.",
    load_file: "Datei laden …",
    file_filter: "Text",
    load_title: "Beschreibung der Ablage laden",
    insert_example: "Beispiel einfügen",
    clear: "Leeren",
    section_off: "Wird nicht übernommen – Haken setzen",
    chars: |n| format!("{n} Zeichen"),
    description_hint: "z. B. „Dateinamen: JJMMTT_Dokumentart_Absender_Betreff …“",

    generate: "Einstellungen erzeugen",
    generate_needs_ai: "Die KI ist ausgeschaltet (Einstellungen › KI)",
    generate_needs_text: "Erst oben die Ablage beschreiben",
    generate_running: "Läuft gerade",
    cancel_hint: "Beim externen Server läuft die Anfrage dort zu Ende; ihr Ergebnis wird verworfen.",
    cancel: "Abbrechen",
    ai_off: "Dafür muss die KI eingeschaltet sein (Reiter KI).",
    ai_status: |s| format!("KI: {s}"),
    applied: "In den Entwurf übernommen – mit „Speichern“ wird er wirksam.",

    proposal: "Vorschlag der KI",
    proposal_hint: "Angehakte Abschnitte kommen in den Entwurf. Was dabei ersetzt würde, steht neben dem Titel.",
    naming: "Dateinamen",
    naming_summary: |date, fallback| format!("Datum {date} · Auffangordner „{fallback}“"),
    replaces_existing: |n| format!("ersetzt {n} bisherige"),
    doctypes: |doctypes, extras| format!("Dokumentarten ({doctypes}) und Zusätze ({extras})"),
    extras: |list| format!("Zusätze: {list}"),
    persons: |n| format!("Personen ({n})"),
    person_from_folder: "Person nur bei Dokumenten in ihren eigenen Ordnern",
    senders_effect: |new, known| format!("{new} neu, {known} ergänzt"),
    senders: |n| format!("Kurznamen für Absender ({n})"),
    notes_effect: |new, existing, missing| {
        join_counts(
            format!("{new} neu"),
            [(existing, format!("{existing} vorhanden")), (missing, format!("{missing} ohne Ordner"))],
        )
    },
    notes: |n| format!("Ordnerbeschreibungen ({n})"),
    only_empty_notes: "Nur leere Beschreibungen füllen – vorhandene bleiben",
    rules: |n| format!("Sonderregeln ({n})"),
    rule_requires: |list| format!(" (nur mit {list})"),
    instructions_effect: "ersetzt die bisherigen",
    instructions: "Anweisungen für die KI",
    apply: "Übernehmen",
    apply_hint: "In den Entwurf; „Speichern“ macht es wirksam",
    discard: "Verwerfen",
};

pub static EN: Texts = Texts {
    template_rejected: |template, unknown| format!("Template “{template}” not applied (unknown parts: {unknown})"),
    date_format_invalid: |bad, kept| format!("Date format “{bad}” is invalid – keeping {kept}"),
    fallback_missing: |wanted, kept| format!("Fallback folder “{wanted}” doesn't exist – keeping “{kept}”"),
    notes_kept: |n| format!("Kept {}", count(n, "existing folder description", "existing folder descriptions")),
    notes_skipped: |n| {
        format!(
            "Skipped {}",
            count(n, "folder description for a missing folder", "folder descriptions for missing folders")
        )
    },
    rule_folder_missing: |name, folder| format!("Rule “{name}”: folder “{folder}” doesn't exist – turned off"),
    rules_replaced: |n| format!("Replaced {}", count(n, "previous special rule", "previous special rules")),

    intro: "Describe your archive in your own words: folders, how file names are built, document types, people, \
            short names and fixed assignments (e.g. “bank statements go to Finance/Bank/Year”). You can also load \
            existing documentation. The local AI turns this into settings that you check before applying them.",
    load_file: "Load file …",
    file_filter: "Text",
    load_title: "Load archive description",
    insert_example: "Insert example",
    clear: "Clear",
    section_off: "Will not be applied – tick the box",
    chars: |n| count(n, "character", "characters"),
    description_hint: "e.g. “File names: YYMMDD_DocumentType_Sender_Subject …”",

    generate: "Generate settings",
    generate_needs_ai: "The AI is switched off (Settings › AI)",
    generate_needs_text: "Describe your filing system above first",
    generate_running: "Already running",
    cancel_hint: "With an external server, the request still runs to the end there; its result is discarded.",
    cancel: "Cancel",
    ai_off: "This needs the AI to be turned on (AI tab).",
    ai_status: |s| format!("AI: {s}"),
    applied: "Applied to the draft – it takes effect when you click “Save”.",

    proposal: "AI suggestion",
    proposal_hint: "Checked sections go into the draft. What they would replace is shown next to the title.",
    naming: "File names",
    naming_summary: |date, fallback| format!("Date {date} · fallback folder “{fallback}”"),
    replaces_existing: |n| format!("replaces {n} existing"),
    doctypes: |doctypes, extras| format!("Document types ({doctypes}) and extras ({extras})"),
    extras: |list| format!("Extras: {list}"),
    persons: |n| format!("People ({n})"),
    person_from_folder: "Person only for documents in their own folders",
    senders_effect: |new, known| format!("{new} new, {known} extended"),
    senders: |n| format!("Short names for senders ({n})"),
    notes_effect: |new, existing, missing| {
        join_counts(
            format!("{new} new"),
            [(existing, format!("{existing} existing")), (missing, format!("{missing} without folder"))],
        )
    },
    notes: |n| format!("Folder descriptions ({n})"),
    only_empty_notes: "Only fill empty descriptions – existing ones stay",
    rules: |n| format!("Special rules ({n})"),
    rule_requires: |list| format!(" (only with {list})"),
    instructions_effect: "replaces the current ones",
    instructions: "Instructions for the AI",
    apply: "Apply",
    apply_hint: "Into the draft; “Save” makes it take effect",
    discard: "Discard",
};

pub static FR: Texts = Texts {
    template_rejected: |template, unknown| {
        format!("Modèle « {template} » non appliqué (éléments inconnus : {unknown})")
    },
    date_format_invalid: |bad, kept| format!("Format de date « {bad} » non valide – {kept} est conservé"),
    fallback_missing: |wanted, kept| format!("Le dossier de repli « {wanted} » n'existe pas – « {kept} » est conservé"),
    notes_kept: |n| {
        count_fr(n, "description de dossier existante conservée", "descriptions de dossiers existantes conservées")
    },
    notes_skipped: |n| {
        count_fr(n, "description de dossier inexistant ignorée", "descriptions de dossiers inexistants ignorées")
    },
    rule_folder_missing: |name, folder| format!("Règle « {name} » : le dossier « {folder} » n'existe pas – désactivée"),
    rules_replaced: |n| count_fr(n, "règle spéciale précédente remplacée", "règles spéciales précédentes remplacées"),

    intro: "Décrivez votre archive avec vos propres mots : dossiers, structure des noms de fichiers, types de \
            document, personnes, noms courts et affectations fixes (p. ex. « relevés bancaires dans \
            Finances/Banque/Année »). Vous pouvez aussi charger une documentation existante. L'IA locale en tire des \
            paramètres que vous vérifiez avant de les appliquer.",
    load_file: "Charger un fichier …",
    file_filter: "Texte",
    load_title: "Charger la description de l'archive",
    insert_example: "Insérer un exemple",
    clear: "Effacer",
    section_off: "Ne sera pas appliqué – cochez la case",
    chars: |n| count_fr(n, "caractère", "caractères"),
    description_hint: "p. ex. « Noms de fichiers : AAMMJJ_TypeDeDocument_Expéditeur_Objet … »",

    generate: "Générer les paramètres",
    generate_needs_ai: "L'IA est désactivée (Paramètres › IA)",
    generate_needs_text: "Décrivez d'abord votre classement ci-dessus",
    generate_running: "Déjà en cours",
    cancel_hint: "Avec un serveur externe, la requête s'y termine quand même ; son résultat est ignoré.",
    cancel: "Annuler",
    ai_off: "Pour cela, l'IA doit être activée (onglet IA).",
    ai_status: |s| format!("IA : {s}"),
    applied: "Appliqué au brouillon – il prend effet avec « Enregistrer ».",

    proposal: "Proposition de l'IA",
    proposal_hint: "Les sections cochées vont dans le brouillon. Ce qu'elles remplaceraient est indiqué à côté du \
                    titre.",
    naming: "Noms de fichiers",
    naming_summary: |date, fallback| format!("Date {date} · dossier de repli « {fallback} »"),
    replaces_existing: |n| format!("remplace {}", count_fr(n, "entrée existante", "entrées existantes")),
    doctypes: |doctypes, extras| format!("Types de document ({doctypes}) et compléments ({extras})"),
    extras: |list| format!("Compléments : {list}"),
    persons: |n| format!("Personnes ({n})"),
    person_from_folder: "Personne uniquement pour les documents de ses propres dossiers",
    senders_effect: |new, known| {
        format!("{}, {}", count_fr(new, "nouveau", "nouveaux"), count_fr(known, "complété", "complétés"))
    },
    senders: |n| format!("Noms courts des expéditeurs ({n})"),
    notes_effect: |new, existing, missing| {
        join_counts(
            count_fr(new, "nouvelle", "nouvelles"),
            [(existing, count_fr(existing, "existante", "existantes")), (missing, format!("{missing} sans dossier"))],
        )
    },
    notes: |n| format!("Descriptions des dossiers ({n})"),
    only_empty_notes: "Ne remplir que les descriptions vides – les existantes restent",
    rules: |n| format!("Règles spéciales ({n})"),
    rule_requires: |list| format!(" (seulement avec {list})"),
    instructions_effect: "remplace les actuelles",
    instructions: "Instructions pour l'IA",
    apply: "Appliquer",
    apply_hint: "Dans le brouillon ; « Enregistrer » le rend effectif",
    discard: "Abandonner",
};

pub static ES: Texts = Texts {
    template_rejected: |template, unknown| {
        format!("Plantilla «{template}» no aplicada (elementos desconocidos: {unknown})")
    },
    date_format_invalid: |bad, kept| format!("Formato de fecha «{bad}» no válido – se mantiene {kept}"),
    fallback_missing: |wanted, kept| format!("La carpeta de reserva «{wanted}» no existe – se mantiene «{kept}»"),
    notes_kept: |n| match n {
        1 => "Se conservó 1 descripción de carpeta existente".to_string(),
        n => format!("Se conservaron {n} descripciones de carpetas existentes"),
    },
    notes_skipped: |n| match n {
        1 => "Se omitió 1 descripción de carpeta inexistente".to_string(),
        n => format!("Se omitieron {n} descripciones de carpetas inexistentes"),
    },
    rule_folder_missing: |name, folder| format!("Regla «{name}»: la carpeta «{folder}» no existe – desactivada"),
    rules_replaced: |n| match n {
        1 => "Se sustituyó 1 regla especial anterior".to_string(),
        n => format!("Se sustituyeron {n} reglas especiales anteriores"),
    },

    intro: "Describe tu archivo con tus propias palabras: carpetas, estructura de los nombres de archivo, tipos de \
            documento, personas, nombres cortos y asignaciones fijas (p. ej. «extractos bancarios a \
            Finanzas/Banco/Año»). También puedes cargar una documentación existente. La IA local crea con ello \
            ajustes que revisas antes de aplicarlos.",
    load_file: "Cargar archivo …",
    file_filter: "Texto",
    load_title: "Cargar la descripción del archivo",
    insert_example: "Insertar ejemplo",
    clear: "Vaciar",
    section_off: "No se aplicará – marque la casilla",
    chars: |n| count(n, "carácter", "caracteres"),
    description_hint: "p. ej. «Nombres de archivo: AAMMDD_TipoDeDocumento_Remitente_Asunto …»",

    generate: "Generar ajustes",
    generate_needs_ai: "La IA está desactivada (Ajustes › IA)",
    generate_needs_text: "Primero describa su archivo arriba",
    generate_running: "Ya en curso",
    cancel_hint: "Con un servidor externo, la solicitud termina allí de todos modos; su resultado se descarta.",
    cancel: "Cancelar",
    ai_off: "Para ello, la IA debe estar activada (pestaña IA).",
    ai_status: |s| format!("IA: {s}"),
    applied: "Aplicado al borrador – se hace efectivo con «Guardar».",

    proposal: "Propuesta de la IA",
    proposal_hint: "Las secciones marcadas pasan al borrador. Lo que sustituirían aparece junto al título.",
    naming: "Nombres de archivo",
    naming_summary: |date, fallback| format!("Fecha {date} · carpeta de reserva «{fallback}»"),
    replaces_existing: |n| format!("sustituye {}", count(n, "existente", "existentes")),
    doctypes: |doctypes, extras| format!("Tipos de documento ({doctypes}) y añadidos ({extras})"),
    extras: |list| format!("Añadidos: {list}"),
    persons: |n| format!("Personas ({n})"),
    person_from_folder: "Persona solo en documentos de sus propias carpetas",
    senders_effect: |new, known| {
        format!("{}, {}", count(new, "nuevo", "nuevos"), count(known, "ampliado", "ampliados"))
    },
    senders: |n| format!("Nombres cortos de remitentes ({n})"),
    notes_effect: |new, existing, missing| {
        join_counts(
            count(new, "nueva", "nuevas"),
            [(existing, count(existing, "existente", "existentes")), (missing, format!("{missing} sin carpeta"))],
        )
    },
    notes: |n| format!("Descripciones de carpetas ({n})"),
    only_empty_notes: "Rellenar solo las descripciones vacías – las existentes se mantienen",
    rules: |n| format!("Reglas especiales ({n})"),
    rule_requires: |list| format!(" (solo con {list})"),
    instructions_effect: "sustituye las actuales",
    instructions: "Instrucciones para la IA",
    apply: "Aplicar",
    apply_hint: "Al borrador; «Guardar» lo hace efectivo",
    discard: "Descartar",
};

pub static IT: Texts = Texts {
    template_rejected: |template, unknown| {
        format!("Modello «{template}» non applicato (elementi sconosciuti: {unknown})")
    },
    date_format_invalid: |bad, kept| format!("Formato data «{bad}» non valido – resta {kept}"),
    fallback_missing: |wanted, kept| format!("La cartella di riserva «{wanted}» non esiste – resta «{kept}»"),
    notes_kept: |n| match n {
        1 => "Mantenuta 1 descrizione di cartella esistente".to_string(),
        n => format!("Mantenute {n} descrizioni di cartelle esistenti"),
    },
    notes_skipped: |n| match n {
        1 => "Saltata 1 descrizione di cartella inesistente".to_string(),
        n => format!("Saltate {n} descrizioni di cartelle inesistenti"),
    },
    rule_folder_missing: |name, folder| format!("Regola «{name}»: la cartella «{folder}» non esiste – disattivata"),
    rules_replaced: |n| match n {
        1 => "Sostituita 1 regola speciale precedente".to_string(),
        n => format!("Sostituite {n} regole speciali precedenti"),
    },

    intro: "Descrivi il tuo archivio con parole tue: cartelle, struttura dei nomi dei file, tipi di documento, \
            persone, nomi brevi e assegnazioni fisse (ad es. «estratti conto in Finanze/Banca/Anno»). Puoi anche \
            caricare una documentazione esistente. L'IA locale ne ricava impostazioni che controlli prima di \
            applicarle.",
    load_file: "Carica file …",
    file_filter: "Testo",
    load_title: "Carica la descrizione dell'archivio",
    insert_example: "Inserisci esempio",
    clear: "Svuota",
    section_off: "Non verrà applicato – spunta la casella",
    chars: |n| count(n, "carattere", "caratteri"),
    description_hint: "ad es. «Nomi dei file: AAMMGG_TipoDiDocumento_Mittente_Oggetto …»",

    generate: "Genera impostazioni",
    generate_needs_ai: "L'IA è disattivata (Impostazioni › IA)",
    generate_needs_text: "Prima descrivi l'archivio qui sopra",
    generate_running: "Già in corso",
    cancel_hint: "Con un server esterno la richiesta termina comunque lì; il risultato viene scartato.",
    cancel: "Annulla",
    ai_off: "Per questo l'IA deve essere attiva (scheda IA).",
    ai_status: |s| format!("IA: {s}"),
    applied: "Applicato alla bozza – diventa effettiva con «Salva».",

    proposal: "Proposta dell'IA",
    proposal_hint: "Le sezioni spuntate vanno nella bozza. Ciò che sostituirebbero è indicato accanto al titolo.",
    naming: "Nomi dei file",
    naming_summary: |date, fallback| format!("Data {date} · cartella di riserva «{fallback}»"),
    replaces_existing: |n| format!("sostituisce {}", count(n, "esistente", "esistenti")),
    doctypes: |doctypes, extras| format!("Tipi di documento ({doctypes}) e aggiunte ({extras})"),
    extras: |list| format!("Aggiunte: {list}"),
    persons: |n| format!("Persone ({n})"),
    person_from_folder: "Persona solo per i documenti nelle sue cartelle",
    senders_effect: |new, known| {
        format!("{}, {}", count(new, "nuovo", "nuovi"), count(known, "integrato", "integrati"))
    },
    senders: |n| format!("Nomi brevi dei mittenti ({n})"),
    notes_effect: |new, existing, missing| {
        join_counts(
            count(new, "nuova", "nuove"),
            [(existing, count(existing, "esistente", "esistenti")), (missing, format!("{missing} senza cartella"))],
        )
    },
    notes: |n| format!("Descrizioni delle cartelle ({n})"),
    only_empty_notes: "Compila solo le descrizioni vuote – quelle esistenti restano",
    rules: |n| format!("Regole speciali ({n})"),
    rule_requires: |list| format!(" (solo con {list})"),
    instructions_effect: "sostituisce quelle attuali",
    instructions: "Istruzioni per l'IA",
    apply: "Applica",
    apply_hint: "Nella bozza; «Salva» lo rende effettivo",
    discard: "Scarta",
};
