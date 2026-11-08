//! Files (`archive.rs`, `ingest.rs`, `tags.rs`): loading, filing, tags, error messages.

pub struct Texts {
    // Filing and undoing (`archive.rs`)
    pub folder_dots: &'static str,
    pub folder_invalid: fn(&str) -> String,
    pub source_missing: fn(&str) -> String,
    pub no_archive_folder: &'static str,
    pub archive_folder_missing: fn(&str) -> String,
    pub folder_not_created: fn(&str) -> String,
    pub copy_failed: fn(&str) -> String,
    /// Path, whether the file is locked.
    pub cannot_move: fn(&str, bool) -> String,
    pub cannot_move_nothing_filed: fn(&str) -> String,
    pub target_missing: fn(&str) -> String,
    pub not_deletable: fn(&str) -> String,
    pub source_taken: fn(&str) -> String,

    // Kind of document (`DocKind::label`)
    pub kind_pdf: &'static str,
    pub kind_image: &'static str,
    pub kind_text: &'static str,
    pub kind_office: &'static str,
    pub kind_legacy_office: &'static str,
    pub kind_other: &'static str,

    // Loading (`ingest.rs`)
    pub loader_stopped: &'static str,
    pub loading_pdfium: &'static str,
    pub no_pdfium_download: &'static str,
    pub library_not_in_package: fn(&str) -> String,
    pub pdfium_not_found: fn(&str) -> String,
    pub not_readable: fn(&str) -> String,
    pub office_damaged: &'static str,
    pub image_format_unknown: &'static str,
    pub image_not_decoded: &'static str,
    pub pdf_not_opened: fn(&str) -> String,
    /// Page number, error.
    pub page_not_rendered: fn(usize, &str) -> String,
    pub pdf_no_pages: &'static str,

    // Tags for Windows Search (`tags.rs`); the reasons follow the message in parentheses
    pub file_not_created: fn(&str) -> String,
    pub replace_failed: &'static str,
    pub file_in_use: fn(&str) -> String,
    pub pdf_not_read: &'static str,
    pub skip_encrypted: &'static str,
    pub skip_signed: &'static str,
    pub pdf_no_catalog: &'static str,
    pub pdf_not_written: &'static str,
    // Rotating pages (`rotate.rs`)
    pub image_not_read: &'static str,
    pub image_not_written: &'static str,
    pub office_not_read: &'static str,
    pub skip_no_properties: &'static str,
    pub skip_unknown_properties: &'static str,
    // Images: only on Windows via the property system
    #[cfg_attr(not(windows), allow(dead_code))]
    pub properties_not_opened: &'static str,
    #[cfg_attr(not(windows), allow(dead_code))]
    pub properties_not_saved: &'static str,
    #[cfg_attr(not(windows), allow(dead_code))]
    pub properties_thread: &'static str,
}

pub fn t() -> &'static Texts {
    super::pick(&DE, &EN, &FR, &ES, &IT)
}

pub static DE: Texts = Texts {
    folder_dots: "Ordnerangabe darf kein '..' enthalten",
    folder_invalid: |f| format!("Ungültige Ordnerangabe '{f}'"),
    source_missing: |p| format!("Quelldatei {p} existiert nicht mehr"),
    no_archive_folder: "Kein Ablageordner konfiguriert",
    archive_folder_missing: |p| format!("Ablageordner {p} existiert nicht"),
    folder_not_created: |p| format!("Ordner {p} kann nicht angelegt werden"),
    copy_failed: |p| format!("Kopieren nach {p} fehlgeschlagen"),
    cannot_move: |p, locked| {
        let hint = if locked { " – ist die Datei noch in einem anderen Programm geöffnet?" } else { "" };
        format!("{p} lässt sich nicht verschieben{hint}")
    },
    cannot_move_nothing_filed: |p| {
        format!(
            "{p} lässt sich nicht verschieben – ist die Datei noch in einem anderen Programm geöffnet? Es wurde nichts \
             abgelegt."
        )
    },
    target_missing: |p| format!("{p} ist nicht mehr da"),
    not_deletable: |p| format!("{p} nicht löschbar"),
    source_taken: |p| format!("Am alten Ort liegt inzwischen wieder eine Datei: {p}"),

    kind_pdf: "PDF",
    kind_image: "Bild",
    kind_text: "Textdatei",
    kind_office: "Office-Dokument",
    kind_legacy_office: "altes Office-Format",
    kind_other: "ohne lesbaren Inhalt",

    loader_stopped: "Der Lade-Thread läuft nicht mehr",
    loading_pdfium: "Lade PDF-Komponente (PDFium, einmalig ca. 4 MB) …",
    no_pdfium_download: "Für dieses System gibt es keinen PDFium-Download",
    library_not_in_package: |lib| format!("{lib} nicht im PDFium-Archiv gefunden"),
    pdfium_not_found: |lib| {
        format!(
            "PDFium-Bibliothek nicht gefunden. Automatischer Download nicht möglich? Dann {lib} von \
             https://github.com/bblanchon/pdfium-binaries neben die Programmdatei legen (oder SCRINIUM_PDFIUM setzen)."
        )
    },
    not_readable: |p| format!("{p} nicht lesbar"),
    office_damaged: "Office-Datei ist beschädigt oder kein ZIP",
    image_format_unknown: "Bildformat nicht erkannt",
    image_not_decoded: "Bild konnte nicht dekodiert werden",
    pdf_not_opened: |e| format!("PDF konnte nicht geöffnet werden: {e}"),
    page_not_rendered: |n, e| format!("Seite {n} konnte nicht gerendert werden: {e}"),
    pdf_no_pages: "Das PDF enthält keine Seiten",

    file_not_created: |p| format!("{p} kann nicht angelegt werden"),
    replace_failed: "Ersetzen fehlgeschlagen",
    file_in_use: |p| format!("{p} ist gerade geöffnet"),
    pdf_not_read: "PDF kann nicht gelesen werden",
    skip_encrypted: "verschlüsseltes PDF",
    skip_signed: "signiertes PDF – die Signatur soll gültig bleiben",
    pdf_no_catalog: "PDF ohne Katalog",
    pdf_not_written: "PDF kann nicht geschrieben werden",
    image_not_read: "Bild kann nicht gelesen werden",
    image_not_written: "Bild kann nicht gespeichert werden",
    office_not_read: "Office-Datei kann nicht gelesen werden",
    skip_no_properties: "Datei ohne Dokumenteigenschaften",
    skip_unknown_properties: "unbekannter Aufbau der Dokumenteigenschaften",
    properties_not_opened: "Eigenschaften lassen sich nicht öffnen",
    properties_not_saved: "Eigenschaften können nicht gespeichert werden",
    properties_thread: "Eigenschaften: Thread abgebrochen",
};

pub static EN: Texts = Texts {
    folder_dots: "The folder path must not contain '..'",
    folder_invalid: |f| format!("Invalid folder path '{f}'"),
    source_missing: |p| format!("Source file {p} no longer exists"),
    no_archive_folder: "No archive folder configured",
    archive_folder_missing: |p| format!("Archive folder {p} does not exist"),
    folder_not_created: |p| format!("Folder {p} cannot be created"),
    copy_failed: |p| format!("Copying to {p} failed"),
    cannot_move: |p, locked| {
        let hint = if locked { " – is the file still open in another program?" } else { "" };
        format!("{p} cannot be moved{hint}")
    },
    cannot_move_nothing_filed: |p| {
        format!("{p} cannot be moved – is the file still open in another program? Nothing was filed.")
    },
    target_missing: |p| format!("{p} is no longer there"),
    not_deletable: |p| format!("{p} cannot be deleted"),
    source_taken: |p| format!("There is a file at the old location again: {p}"),

    kind_pdf: "PDF",
    kind_image: "Image",
    kind_text: "Text file",
    kind_office: "Office document",
    kind_legacy_office: "old Office format",
    kind_other: "no readable content",

    loader_stopped: "The loading thread is no longer running",
    loading_pdfium: "Downloading the PDF component (PDFium, one time, about 4 MB) …",
    no_pdfium_download: "There is no PDFium download for this system",
    library_not_in_package: |lib| format!("{lib} not found in the PDFium package"),
    pdfium_not_found: |lib| {
        format!(
            "PDFium library not found. Automatic download not possible? Then put {lib} from \
             https://github.com/bblanchon/pdfium-binaries next to the program file (or set SCRINIUM_PDFIUM)."
        )
    },
    not_readable: |p| format!("{p} cannot be read"),
    office_damaged: "The Office file is damaged or not a ZIP file",
    image_format_unknown: "Image format not recognised",
    image_not_decoded: "The image could not be decoded",
    pdf_not_opened: |e| format!("The PDF could not be opened: {e}"),
    page_not_rendered: |n, e| format!("Page {n} could not be rendered: {e}"),
    pdf_no_pages: "The PDF has no pages",

    file_not_created: |p| format!("{p} cannot be created"),
    replace_failed: "Replacing the file failed",
    file_in_use: |p| format!("{p} is currently open"),
    pdf_not_read: "The PDF cannot be read",
    skip_encrypted: "encrypted PDF",
    skip_signed: "signed PDF – the signature should stay valid",
    pdf_no_catalog: "PDF without catalog",
    pdf_not_written: "The PDF cannot be written",
    image_not_read: "The image cannot be read",
    image_not_written: "The image cannot be saved",
    office_not_read: "The Office file cannot be read",
    skip_no_properties: "file without document properties",
    skip_unknown_properties: "unknown structure of the document properties",
    properties_not_opened: "The properties cannot be opened",
    properties_not_saved: "The properties cannot be saved",
    properties_thread: "Properties: thread aborted",
};

pub static FR: Texts = Texts {
    folder_dots: "Le chemin du dossier ne doit pas contenir '..'",
    folder_invalid: |f| format!("Chemin de dossier non valide '{f}'"),
    source_missing: |p| format!("Le fichier source {p} n'existe plus"),
    no_archive_folder: "Aucun dossier d'archive configuré",
    archive_folder_missing: |p| format!("Le dossier d'archive {p} n'existe pas"),
    folder_not_created: |p| format!("Impossible de créer le dossier {p}"),
    copy_failed: |p| format!("La copie vers {p} a échoué"),
    cannot_move: |p, locked| {
        let hint = if locked { " – le fichier est-il encore ouvert dans un autre programme ?" } else { "" };
        format!("Impossible de déplacer {p}{hint}")
    },
    cannot_move_nothing_filed: |p| {
        format!(
            "Impossible de déplacer {p} – le fichier est-il encore ouvert dans un autre programme ? Rien n'a été \
             classé."
        )
    },
    target_missing: |p| format!("{p} n'est plus là"),
    not_deletable: |p| format!("Impossible de supprimer {p}"),
    source_taken: |p| format!("Un fichier se trouve de nouveau à l'ancien emplacement : {p}"),

    kind_pdf: "PDF",
    kind_image: "Image",
    kind_text: "Fichier texte",
    kind_office: "Document Office",
    kind_legacy_office: "ancien format Office",
    kind_other: "sans contenu lisible",

    loader_stopped: "Le thread de chargement ne tourne plus",
    loading_pdfium: "Téléchargement du composant PDF (PDFium, une seule fois, env. 4 Mo) …",
    no_pdfium_download: "Aucun téléchargement de PDFium n'existe pour ce système",
    library_not_in_package: |lib| format!("{lib} introuvable dans le paquet PDFium"),
    pdfium_not_found: |lib| {
        format!(
            "Bibliothèque PDFium introuvable. Le téléchargement automatique est impossible ? Placez alors {lib} de \
             https://github.com/bblanchon/pdfium-binaries à côté du fichier du programme (ou définissez \
             SCRINIUM_PDFIUM)."
        )
    },
    not_readable: |p| format!("Impossible de lire {p}"),
    office_damaged: "Le fichier Office est endommagé ou n'est pas un ZIP",
    image_format_unknown: "Format d'image non reconnu",
    image_not_decoded: "Impossible de décoder l'image",
    pdf_not_opened: |e| format!("Impossible d'ouvrir le PDF : {e}"),
    page_not_rendered: |n, e| format!("Impossible de générer le rendu de la page {n} : {e}"),
    pdf_no_pages: "Le PDF ne contient aucune page",

    file_not_created: |p| format!("Impossible de créer {p}"),
    replace_failed: "Le remplacement du fichier a échoué",
    file_in_use: |p| format!("{p} est actuellement ouvert"),
    pdf_not_read: "Impossible de lire le PDF",
    skip_encrypted: "PDF chiffré",
    skip_signed: "PDF signé – la signature doit rester valide",
    pdf_no_catalog: "PDF sans catalogue",
    pdf_not_written: "Impossible d'écrire le PDF",
    image_not_read: "Impossible de lire l'image",
    image_not_written: "Impossible d'enregistrer l'image",
    office_not_read: "Impossible de lire le fichier Office",
    skip_no_properties: "fichier sans propriétés du document",
    skip_unknown_properties: "structure inconnue des propriétés du document",
    properties_not_opened: "Impossible d'ouvrir les propriétés",
    properties_not_saved: "Impossible d'enregistrer les propriétés",
    properties_thread: "Propriétés : thread interrompu",
};

pub static ES: Texts = Texts {
    folder_dots: "La ruta de la carpeta no puede contener '..'",
    folder_invalid: |f| format!("Ruta de carpeta no válida '{f}'"),
    source_missing: |p| format!("El archivo de origen {p} ya no existe"),
    no_archive_folder: "No hay ninguna carpeta del archivo configurada",
    archive_folder_missing: |p| format!("La carpeta del archivo {p} no existe"),
    folder_not_created: |p| format!("No se puede crear la carpeta {p}"),
    copy_failed: |p| format!("Error al copiar a {p}"),
    cannot_move: |p, locked| {
        let hint = if locked { " – ¿sigue abierto el archivo en otro programa?" } else { "" };
        format!("No se puede mover {p}{hint}")
    },
    cannot_move_nothing_filed: |p| {
        format!("No se puede mover {p} – ¿sigue abierto el archivo en otro programa? No se ha archivado nada.")
    },
    target_missing: |p| format!("{p} ya no está"),
    not_deletable: |p| format!("No se puede eliminar {p}"),
    source_taken: |p| format!("Vuelve a haber un archivo en la ubicación anterior: {p}"),

    kind_pdf: "PDF",
    kind_image: "Imagen",
    kind_text: "Archivo de texto",
    kind_office: "Documento de Office",
    kind_legacy_office: "formato antiguo de Office",
    kind_other: "sin contenido legible",

    loader_stopped: "El hilo de carga ya no está en ejecución",
    loading_pdfium: "Descargando el componente PDF (PDFium, una sola vez, unos 4 MB) …",
    no_pdfium_download: "No hay descarga de PDFium para este sistema",
    library_not_in_package: |lib| format!("No se encontró {lib} en el paquete de PDFium"),
    pdfium_not_found: |lib| {
        format!(
            "No se encontró la biblioteca PDFium. ¿No es posible la descarga automática? Entonces coloca {lib} de \
             https://github.com/bblanchon/pdfium-binaries junto al archivo del programa (o define SCRINIUM_PDFIUM)."
        )
    },
    not_readable: |p| format!("No se puede leer {p}"),
    office_damaged: "El archivo de Office está dañado o no es un ZIP",
    image_format_unknown: "Formato de imagen no reconocido",
    image_not_decoded: "No se pudo decodificar la imagen",
    pdf_not_opened: |e| format!("No se pudo abrir el PDF: {e}"),
    page_not_rendered: |n, e| format!("No se pudo renderizar la página {n}: {e}"),
    pdf_no_pages: "El PDF no contiene páginas",

    file_not_created: |p| format!("No se puede crear {p}"),
    replace_failed: "Error al reemplazar el archivo",
    file_in_use: |p| format!("{p} está abierto en este momento"),
    pdf_not_read: "No se puede leer el PDF",
    skip_encrypted: "PDF cifrado",
    skip_signed: "PDF firmado – la firma debe seguir siendo válida",
    pdf_no_catalog: "PDF sin catálogo",
    pdf_not_written: "No se puede escribir el PDF",
    image_not_read: "No se puede leer la imagen",
    image_not_written: "No se puede guardar la imagen",
    office_not_read: "No se puede leer el archivo de Office",
    skip_no_properties: "archivo sin propiedades del documento",
    skip_unknown_properties: "estructura desconocida de las propiedades del documento",
    properties_not_opened: "No se pueden abrir las propiedades",
    properties_not_saved: "No se pueden guardar las propiedades",
    properties_thread: "Propiedades: hilo interrumpido",
};

pub static IT: Texts = Texts {
    folder_dots: "Il percorso della cartella non può contenere '..'",
    folder_invalid: |f| format!("Percorso della cartella non valido '{f}'"),
    source_missing: |p| format!("Il file di origine {p} non esiste più"),
    no_archive_folder: "Nessuna cartella dell'archivio configurata",
    archive_folder_missing: |p| format!("La cartella dell'archivio {p} non esiste"),
    folder_not_created: |p| format!("Impossibile creare la cartella {p}"),
    copy_failed: |p| format!("Copia in {p} non riuscita"),
    cannot_move: |p, locked| {
        let hint = if locked { " – il file è ancora aperto in un altro programma?" } else { "" };
        format!("Impossibile spostare {p}{hint}")
    },
    cannot_move_nothing_filed: |p| {
        format!(
            "Impossibile spostare {p} – il file è ancora aperto in un altro programma? Non è stato archiviato nulla."
        )
    },
    target_missing: |p| format!("{p} non c'è più"),
    not_deletable: |p| format!("Impossibile eliminare {p}"),
    source_taken: |p| format!("Nella posizione precedente c'è di nuovo un file: {p}"),

    kind_pdf: "PDF",
    kind_image: "Immagine",
    kind_text: "File di testo",
    kind_office: "Documento Office",
    kind_legacy_office: "vecchio formato Office",
    kind_other: "senza contenuto leggibile",

    loader_stopped: "Il thread di caricamento non è più in esecuzione",
    loading_pdfium: "Scaricamento del componente PDF (PDFium, una sola volta, circa 4 MB) …",
    no_pdfium_download: "Non esiste un download di PDFium per questo sistema",
    library_not_in_package: |lib| format!("{lib} non trovato nel pacchetto PDFium"),
    pdfium_not_found: |lib| {
        format!(
            "Libreria PDFium non trovata. Il download automatico non è possibile? Allora metti {lib} da \
             https://github.com/bblanchon/pdfium-binaries accanto al file del programma (o imposta SCRINIUM_PDFIUM)."
        )
    },
    not_readable: |p| format!("Impossibile leggere {p}"),
    office_damaged: "Il file Office è danneggiato o non è uno ZIP",
    image_format_unknown: "Formato immagine non riconosciuto",
    image_not_decoded: "Impossibile decodificare l'immagine",
    pdf_not_opened: |e| format!("Impossibile aprire il PDF: {e}"),
    page_not_rendered: |n, e| format!("Impossibile eseguire il rendering della pagina {n}: {e}"),
    pdf_no_pages: "Il PDF non contiene pagine",

    file_not_created: |p| format!("Impossibile creare {p}"),
    replace_failed: "Sostituzione del file non riuscita",
    file_in_use: |p| format!("{p} è attualmente aperto"),
    pdf_not_read: "Impossibile leggere il PDF",
    skip_encrypted: "PDF crittografato",
    skip_signed: "PDF firmato – la firma deve restare valida",
    pdf_no_catalog: "PDF senza catalogo",
    pdf_not_written: "Impossibile scrivere il PDF",
    image_not_read: "Impossibile leggere l'immagine",
    image_not_written: "Impossibile salvare l'immagine",
    office_not_read: "Impossibile leggere il file Office",
    skip_no_properties: "file senza proprietà del documento",
    skip_unknown_properties: "struttura sconosciuta delle proprietà del documento",
    properties_not_opened: "Impossibile aprire le proprietà",
    properties_not_saved: "Impossibile salvare le proprietà",
    properties_thread: "Proprietà: thread interrotto",
};
