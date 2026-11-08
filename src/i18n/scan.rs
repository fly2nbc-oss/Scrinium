//! Scanning (`about.rs` Scan tab, `scan.rs`, `twain.rs`, `scan_pdf.rs`): scanner, post-processing, messages.

/// On Linux the texts for TWAIN and WIA stay unused, on Windows the three for SANE.
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Texts {
    // Scan tab: sections
    pub scanner: &'static str,
    pub preparation: &'static str,

    // Scanner section
    pub searching: &'static str,
    pub none_found_windows: &'static str,
    pub none_found_linux: &'static str,
    pub help_windows: &'static str,
    pub help_linux: &'static str,
    pub auto_windows: &'static str,
    pub auto_linux: &'static str,
    pub not_found: fn(&str) -> String,
    pub save_in: &'static str,
    pub save_in_hint: &'static str,
    pub browse: &'static str,
    pub folder_title: &'static str,
    pub resolution: &'static str,
    pub color: &'static str,
    pub color_scan: &'static str,

    // Post-processing section
    pub preparation_help: &'static str,
    pub color_mode: &'static str,
    pub text: &'static str,
    pub text_layer: &'static str,

    // Finding scanners and scanning (WIA, SANE)
    pub wia_device: fn(&str) -> String,
    pub powershell_failed: &'static str,
    pub scan_unreadable: &'static str,
    pub wia_no_scanner: &'static str,
    pub wia_unreachable: &'static str,
    pub wia_busy: &'static str,
    pub wia_no_paper: &'static str,
    pub wia_paper_jam: &'static str,
    pub wia_device_error: &'static str,
    pub wia_missing: &'static str,
    pub wia_no_answer: &'static str,
    #[cfg_attr(windows, allow(dead_code))]
    pub sane_missing: &'static str,
    #[cfg_attr(windows, allow(dead_code))]
    pub scanimage_failed: &'static str,
    #[cfg_attr(windows, allow(dead_code))]
    pub scan_failed: fn(&str) -> String,

    // TWAIN
    pub twain_manager_failed: fn(u16) -> String,
    pub twain_open_failed: fn(&str, u16) -> String,
    pub twain_dialog_failed: fn(u16) -> String,
    pub twain_transfer_failed: fn(u16) -> String,
    pub twain_empty_image: &'static str,
    pub twain_image_unreadable: &'static str,
    pub twain_image_short: &'static str,
    pub twain_image_invalid: fn(&str) -> String,

    // Post-processing: progress, hints, errors
    pub worker_gone: &'static str,
    pub step_ocr: &'static str,
    pub note_no_ocr: fn(&str) -> String,
    pub step_page: fn(usize, usize) -> String,
    pub note_page_raw: fn(usize, &str) -> String,
    pub step_save: &'static str,
    pub note_ocr_failed: fn(&str) -> String,
    pub no_pages: &'static str,
    pub cannot_create: fn(&str) -> String,
}

pub fn t() -> &'static Texts {
    super::pick(&DE, &EN, &FR, &ES, &IT)
}

pub static DE: Texts = Texts {
    scanner: "Scanner",
    preparation: "Aufbereitung",

    searching: "Scanner werden gesucht …",
    none_found_windows: "Kein Scanner gefunden. Ein Scanner mit TWAIN-Treiber des Herstellers (z. B. Epson Scan 2) \
                         oder einer, den Windows kennt, erscheint hier.",
    none_found_linux: "Kein Scanner gefunden. Scrinium scannt über SANE (Paket sane-utils, Prüfen mit „scanimage -L“).",
    help_windows: "Ein TWAIN-Scanner öffnet den Dialog des Herstellers mit allen seinen Optionen; alle Blätter aus dem \
                   Einzug kommen in einem Durchgang. Der Windows-Dialog ist für Scanner ohne TWAIN-Treiber.",
    help_linux: "Scrinium scannt über SANE mit diesen Einstellungen; jeder Scan wird ein PDF, mit Einzug alle Seiten zusammen.",
    auto_windows: "Automatisch – TWAIN, sonst Windows-Dialog",
    auto_linux: "Automatisch – erster Scanner",
    not_found: |d| format!("{d} (nicht gefunden)"),
    save_in: "Speichern in",
    save_in_hint: "Hier liegen Scans, bis sie abgelegt sind. Leer = der Download-Ordner.",
    browse: "Durchsuchen …",
    folder_title: "Ordner für Scans",
    resolution: "Auflösung",
    color: "Farbe",
    color_scan: "In Farbe scannen (sonst Graustufen)",

    preparation_help: "Nach dem Scannen rückt Scrinium die Seiten gerade und speichert ein PDF, in dem sich suchen und markieren lässt. Die Farbart wählst du auch gleich neben „Scannen“; sie gilt für alle neuen Scans.",
    color_mode: "Farbart",
    text: "Text",
    text_layer: "Text erkennen und unsichtbar ins PDF legen (durchsuchbar)",

    wia_device: |names| format!("Windows-Dialog (WIA): {names}"),
    powershell_failed: "PowerShell ließ sich nicht starten",
    scan_unreadable: "Der Scan ließ sich nicht lesen",
    wia_no_scanner: "Kein Scanner gefunden – ist er eingeschaltet und verbunden?",
    wia_unreachable: "Der Scanner ist nicht erreichbar.",
    wia_busy: "Der Scanner ist gerade beschäftigt.",
    wia_no_paper: "Kein Papier im Einzug.",
    wia_paper_jam: "Papierstau im Scanner.",
    wia_device_error: "Der Scanner meldet einen Fehler.",
    wia_missing: "Die Scan-Funktion von Windows (WIA) fehlt auf diesem Rechner.",
    wia_no_answer: "Der Scanner hat nicht geantwortet.",
    sane_missing: "Zum Scannen wird SANE gebraucht (scanimage, z. B. „apt install sane-utils“).",
    scanimage_failed: "scanimage ließ sich nicht starten",
    scan_failed: |e| format!("Scannen fehlgeschlagen: {e}"),

    twain_manager_failed: |rc| format!("Der TWAIN-Manager ließ sich nicht öffnen (Code {rc})"),
    twain_open_failed: |name, rc| format!("Der Scanner „{name}“ ließ sich nicht öffnen (TWAIN-Code {rc})"),
    twain_dialog_failed: |rc| format!("Der Dialog des Scanners ließ sich nicht öffnen (TWAIN-Code {rc})"),
    twain_transfer_failed: |rc| format!("Der Scanner hat die Seite nicht übergeben (TWAIN-Code {rc})"),
    twain_empty_image: "Der Scanner hat ein leeres Bild übergeben",
    twain_image_unreadable: "Das Bild des Scanners ist nicht lesbar",
    twain_image_short: "Bild des Scanners zu kurz",
    twain_image_invalid: |e| format!("Das Bild des Scanners ließ sich nicht lesen: {e}"),

    worker_gone: "Die Aufbereitung läuft nicht mehr",
    step_ocr: "Texterkennung starten …",
    note_no_ocr: |e| format!("ohne Texterkennung ({e})"),
    step_page: |i, n| format!("Scan aufbereiten – Seite {i}/{n} …"),
    note_page_raw: |i, e| format!("Seite {i}: unbearbeitet ({e})"),
    step_save: "PDF speichern …",
    note_ocr_failed: |e| format!("Texterkennung fehlgeschlagen ({e})"),
    no_pages: "Keine Seiten gescannt",
    cannot_create: |path| format!("{path} ließ sich nicht anlegen"),
};

pub static EN: Texts = Texts {
    scanner: "Scanner",
    preparation: "Processing",

    searching: "Looking for scanners …",
    none_found_windows: "No scanner found. A scanner with the manufacturer's TWAIN driver (e.g. Epson Scan 2) or one \
                         that Windows knows appears here.",
    none_found_linux: "No scanner found. Scrinium scans via SANE (package sane-utils, check with “scanimage -L”).",
    help_windows: "A TWAIN scanner opens the manufacturer's dialog with all its options; all sheets from the feeder \
                   come in one pass. The Windows dialog is for scanners without a TWAIN driver.",
    help_linux: "Scrinium scans via SANE with these settings; each scan becomes one PDF, with a feeder all pages together.",
    auto_windows: "Automatic – TWAIN, otherwise Windows dialog",
    auto_linux: "Automatic – first scanner",
    not_found: |d| format!("{d} (not found)"),
    save_in: "Save in",
    save_in_hint: "Scans stay here until they are filed. Empty = the Downloads folder.",
    browse: "Browse …",
    folder_title: "Folder for scans",
    resolution: "Resolution",
    color: "Colour",
    color_scan: "Scan in colour (otherwise greyscale)",

    preparation_help: "After scanning, Scrinium straightens the pages and saves a PDF in which you can search and select text. You can also pick the colour mode right next to “Scan”; it applies to all new scans.",
    color_mode: "Colour mode",
    text: "Text",
    text_layer: "Recognise text and place it invisibly in the PDF (searchable)",

    wia_device: |names| format!("Windows dialog (WIA): {names}"),
    powershell_failed: "Could not start PowerShell",
    scan_unreadable: "Could not read the scan",
    wia_no_scanner: "No scanner found – is it switched on and connected?",
    wia_unreachable: "The scanner cannot be reached.",
    wia_busy: "The scanner is busy right now.",
    wia_no_paper: "No paper in the feeder.",
    wia_paper_jam: "Paper jam in the scanner.",
    wia_device_error: "The scanner reports an error.",
    wia_missing: "The Windows scan feature (WIA) is missing on this computer.",
    wia_no_answer: "The scanner did not respond.",
    sane_missing: "Scanning needs SANE (scanimage, e.g. “apt install sane-utils”).",
    scanimage_failed: "Could not start scanimage",
    scan_failed: |e| format!("Scanning failed: {e}"),

    twain_manager_failed: |rc| format!("Could not open the TWAIN manager (code {rc})"),
    twain_open_failed: |name, rc| format!("Could not open the scanner “{name}” (TWAIN code {rc})"),
    twain_dialog_failed: |rc| format!("Could not open the scanner's dialog (TWAIN code {rc})"),
    twain_transfer_failed: |rc| format!("The scanner did not hand over the page (TWAIN code {rc})"),
    twain_empty_image: "The scanner handed over an empty image",
    twain_image_unreadable: "The scanner's image is not readable",
    twain_image_short: "Scanner image too short",
    twain_image_invalid: |e| format!("Could not read the scanner's image: {e}"),

    worker_gone: "Processing is no longer running",
    step_ocr: "Starting text recognition …",
    note_no_ocr: |e| format!("without text recognition ({e})"),
    step_page: |i, n| format!("Processing scan – page {i}/{n} …"),
    note_page_raw: |i, e| format!("Page {i}: not processed ({e})"),
    step_save: "Saving PDF …",
    note_ocr_failed: |e| format!("Text recognition failed ({e})"),
    no_pages: "No pages scanned",
    cannot_create: |path| format!("Could not create {path}"),
};

pub static FR: Texts = Texts {
    scanner: "Scanner",
    preparation: "Traitement",

    searching: "Recherche des scanners …",
    none_found_windows: "Aucun scanner trouvé. Un scanner doté du pilote TWAIN du fabricant (p. ex. Epson Scan 2) ou \
                         reconnu par Windows apparaît ici.",
    none_found_linux: "Aucun scanner trouvé. Scrinium numérise via SANE (paquet sane-utils, vérifier avec « scanimage \
                       -L »).",
    help_windows: "Un scanner TWAIN ouvre la boîte de dialogue du fabricant avec toutes ses options ; toutes les \
                   feuilles du chargeur arrivent en un seul passage. La boîte de dialogue Windows sert aux scanners \
                   sans pilote TWAIN.",
    help_linux: "Scrinium numérise via SANE avec ces réglages ; chaque numérisation devient un PDF, avec chargeur toutes les pages ensemble.",
    auto_windows: "Automatique – TWAIN, sinon boîte de dialogue Windows",
    auto_linux: "Automatique – premier scanner",
    not_found: |d| format!("{d} (introuvable)"),
    save_in: "Enregistrer dans",
    save_in_hint: "Les numérisations restent ici jusqu'à leur classement. Vide = le dossier Téléchargements.",
    browse: "Parcourir …",
    folder_title: "Dossier des numérisations",
    resolution: "Résolution",
    color: "Couleur",
    color_scan: "Numériser en couleur (sinon en niveaux de gris)",

    preparation_help: "Après la numérisation, Scrinium redresse les pages et enregistre un PDF dans lequel vous pouvez rechercher et sélectionner du texte. Vous pouvez aussi choisir le mode couleur juste à côté de « Numériser » ; il s'applique à toutes les nouvelles numérisations.",
    color_mode: "Mode couleur",
    text: "Texte",
    text_layer: "Reconnaître le texte et le placer de façon invisible dans le PDF (avec recherche de texte)",

    wia_device: |names| format!("Boîte de dialogue Windows (WIA) : {names}"),
    powershell_failed: "Impossible de démarrer PowerShell",
    scan_unreadable: "Impossible de lire la numérisation",
    wia_no_scanner: "Aucun scanner trouvé – est-il allumé et connecté ?",
    wia_unreachable: "Le scanner est injoignable.",
    wia_busy: "Le scanner est occupé pour le moment.",
    wia_no_paper: "Pas de papier dans le chargeur.",
    wia_paper_jam: "Bourrage papier dans le scanner.",
    wia_device_error: "Le scanner signale une erreur.",
    wia_missing: "La fonction de numérisation de Windows (WIA) est absente de cet ordinateur.",
    wia_no_answer: "Le scanner n'a pas répondu.",
    sane_missing: "La numérisation nécessite SANE (scanimage, p. ex. « apt install sane-utils »).",
    scanimage_failed: "Impossible de démarrer scanimage",
    scan_failed: |e| format!("Échec de la numérisation : {e}"),

    twain_manager_failed: |rc| format!("Impossible d'ouvrir le gestionnaire TWAIN (code {rc})"),
    twain_open_failed: |name, rc| format!("Impossible d'ouvrir le scanner « {name} » (code TWAIN {rc})"),
    twain_dialog_failed: |rc| format!("Impossible d'ouvrir la boîte de dialogue du scanner (code TWAIN {rc})"),
    twain_transfer_failed: |rc| format!("Le scanner n'a pas transmis la page (code TWAIN {rc})"),
    twain_empty_image: "Le scanner a transmis une image vide",
    twain_image_unreadable: "L'image du scanner est illisible",
    twain_image_short: "Image du scanner trop courte",
    twain_image_invalid: |e| format!("Impossible de lire l'image du scanner : {e}"),

    worker_gone: "Le traitement ne fonctionne plus",
    step_ocr: "Démarrage de la reconnaissance de texte …",
    note_no_ocr: |e| format!("sans reconnaissance de texte ({e})"),
    step_page: |i, n| format!("Traitement de la numérisation – page {i}/{n} …"),
    note_page_raw: |i, e| format!("Page {i} : non traitée ({e})"),
    step_save: "Enregistrement du PDF …",
    note_ocr_failed: |e| format!("Échec de la reconnaissance de texte ({e})"),
    no_pages: "Aucune page numérisée",
    cannot_create: |path| format!("Impossible de créer {path}"),
};

pub static ES: Texts = Texts {
    scanner: "Escáner",
    preparation: "Procesado",

    searching: "Buscando escáneres …",
    none_found_windows: "No se ha encontrado ningún escáner. Aquí aparece un escáner con el controlador TWAIN del \
                         fabricante (p. ej. Epson Scan 2) o uno que Windows reconozca.",
    none_found_linux: "No se ha encontrado ningún escáner. Scrinium escanea mediante SANE (paquete sane-utils, \
                       compruébalo con «scanimage -L»).",
    help_windows: "Un escáner TWAIN abre el diálogo del fabricante con todas sus opciones; todas las hojas del \
                   alimentador llegan de una vez. El diálogo de Windows es para escáneres sin controlador TWAIN.",
    help_linux: "Scrinium escanea mediante SANE con estos ajustes; cada escaneo es un PDF, con alimentador todas las páginas juntas.",
    auto_windows: "Automático – TWAIN, si no, diálogo de Windows",
    auto_linux: "Automático – primer escáner",
    not_found: |d| format!("{d} (no encontrado)"),
    save_in: "Guardar en",
    save_in_hint: "Aquí quedan los escaneos hasta que se archivan. Vacío = la carpeta Descargas.",
    browse: "Examinar …",
    folder_title: "Carpeta para escaneos",
    resolution: "Resolución",
    color: "Color",
    color_scan: "Escanear en color (si no, en escala de grises)",

    preparation_help: "Después de escanear, Scrinium endereza las páginas y guarda un PDF en el que puedes buscar y seleccionar texto. El modo de color también se elige junto a «Escanear»; se aplica a todos los escaneos nuevos.",
    color_mode: "Modo de color",
    text: "Texto",
    text_layer: "Reconocer el texto e incluirlo de forma invisible en el PDF (con búsqueda)",

    wia_device: |names| format!("Diálogo de Windows (WIA): {names}"),
    powershell_failed: "No se pudo iniciar PowerShell",
    scan_unreadable: "No se pudo leer el escaneo",
    wia_no_scanner: "No se ha encontrado ningún escáner – ¿está encendido y conectado?",
    wia_unreachable: "No se puede acceder al escáner.",
    wia_busy: "El escáner está ocupado en este momento.",
    wia_no_paper: "No hay papel en el alimentador.",
    wia_paper_jam: "Atasco de papel en el escáner.",
    wia_device_error: "El escáner informa de un error.",
    wia_missing: "La función de escaneo de Windows (WIA) no está disponible en este equipo.",
    wia_no_answer: "El escáner no ha respondido.",
    sane_missing: "Para escanear se necesita SANE (scanimage, p. ej. «apt install sane-utils»).",
    scanimage_failed: "No se pudo iniciar scanimage",
    scan_failed: |e| format!("Error al escanear: {e}"),

    twain_manager_failed: |rc| format!("No se pudo abrir el gestor TWAIN (código {rc})"),
    twain_open_failed: |name, rc| format!("No se pudo abrir el escáner «{name}» (código TWAIN {rc})"),
    twain_dialog_failed: |rc| format!("No se pudo abrir el diálogo del escáner (código TWAIN {rc})"),
    twain_transfer_failed: |rc| format!("El escáner no ha entregado la página (código TWAIN {rc})"),
    twain_empty_image: "El escáner ha entregado una imagen vacía",
    twain_image_unreadable: "La imagen del escáner no se puede leer",
    twain_image_short: "Imagen del escáner demasiado corta",
    twain_image_invalid: |e| format!("No se pudo leer la imagen del escáner: {e}"),

    worker_gone: "El procesado ya no está en marcha",
    step_ocr: "Iniciando el reconocimiento de texto …",
    note_no_ocr: |e| format!("sin reconocimiento de texto ({e})"),
    step_page: |i, n| format!("Procesando el escaneo – página {i}/{n} …"),
    note_page_raw: |i, e| format!("Página {i}: sin procesar ({e})"),
    step_save: "Guardando el PDF …",
    note_ocr_failed: |e| format!("Error en el reconocimiento de texto ({e})"),
    no_pages: "No se ha escaneado ninguna página",
    cannot_create: |path| format!("No se pudo crear {path}"),
};

pub static IT: Texts = Texts {
    scanner: "Scanner",
    preparation: "Elaborazione",

    searching: "Ricerca degli scanner …",
    none_found_windows: "Nessuno scanner trovato. Qui compare uno scanner con il driver TWAIN del produttore (ad es. \
                         Epson Scan 2) o uno riconosciuto da Windows.",
    none_found_linux: "Nessuno scanner trovato. Scrinium scansiona tramite SANE (pacchetto sane-utils, verifica con \
                       «scanimage -L»).",
    help_windows: "Uno scanner TWAIN apre la finestra del produttore con tutte le sue opzioni; tutti i fogli \
                   dell'alimentatore arrivano in un solo passaggio. La finestra di Windows è per gli scanner senza \
                   driver TWAIN.",
    help_linux: "Scrinium scansiona tramite SANE con queste impostazioni; ogni scansione diventa un PDF, con alimentatore tutte le pagine insieme.",
    auto_windows: "Automatico – TWAIN, altrimenti finestra di Windows",
    auto_linux: "Automatico – primo scanner",
    not_found: |d| format!("{d} (non trovato)"),
    save_in: "Salva in",
    save_in_hint: "Qui restano le scansioni finché non vengono archiviate. Vuoto = la cartella Download.",
    browse: "Sfoglia …",
    folder_title: "Cartella per le scansioni",
    resolution: "Risoluzione",
    color: "Colore",
    color_scan: "Scansiona a colori (altrimenti in scala di grigi)",

    preparation_help: "Dopo la scansione, Scrinium raddrizza le pagine e salva un PDF in cui puoi cercare e selezionare il testo. La modalità colore si sceglie anche accanto a «Scansiona»; vale per tutte le nuove scansioni.",
    color_mode: "Modalità colore",
    text: "Testo",
    text_layer: "Riconosci il testo e inseriscilo invisibile nel PDF (ricercabile)",

    wia_device: |names| format!("Finestra di Windows (WIA): {names}"),
    powershell_failed: "Impossibile avviare PowerShell",
    scan_unreadable: "Impossibile leggere la scansione",
    wia_no_scanner: "Nessuno scanner trovato – è acceso e collegato?",
    wia_unreachable: "Lo scanner non è raggiungibile.",
    wia_busy: "Lo scanner è occupato in questo momento.",
    wia_no_paper: "Nessun foglio nell'alimentatore.",
    wia_paper_jam: "Inceppamento della carta nello scanner.",
    wia_device_error: "Lo scanner segnala un errore.",
    wia_missing: "La funzione di scansione di Windows (WIA) manca su questo computer.",
    wia_no_answer: "Lo scanner non ha risposto.",
    sane_missing: "Per scansionare serve SANE (scanimage, ad es. «apt install sane-utils»).",
    scanimage_failed: "Impossibile avviare scanimage",
    scan_failed: |e| format!("Scansione non riuscita: {e}"),

    twain_manager_failed: |rc| format!("Impossibile aprire il gestore TWAIN (codice {rc})"),
    twain_open_failed: |name, rc| format!("Impossibile aprire lo scanner «{name}» (codice TWAIN {rc})"),
    twain_dialog_failed: |rc| format!("Impossibile aprire la finestra dello scanner (codice TWAIN {rc})"),
    twain_transfer_failed: |rc| format!("Lo scanner non ha trasmesso la pagina (codice TWAIN {rc})"),
    twain_empty_image: "Lo scanner ha trasmesso un'immagine vuota",
    twain_image_unreadable: "L'immagine dello scanner non è leggibile",
    twain_image_short: "Immagine dello scanner troppo corta",
    twain_image_invalid: |e| format!("Impossibile leggere l'immagine dello scanner: {e}"),

    worker_gone: "L'elaborazione non è più attiva",
    step_ocr: "Avvio del riconoscimento del testo …",
    note_no_ocr: |e| format!("senza riconoscimento del testo ({e})"),
    step_page: |i, n| format!("Elaborazione della scansione – pagina {i}/{n} …"),
    note_page_raw: |i, e| format!("Pagina {i}: non elaborata ({e})"),
    step_save: "Salvataggio del PDF …",
    note_ocr_failed: |e| format!("Riconoscimento del testo non riuscito ({e})"),
    no_pages: "Nessuna pagina scansionata",
    cannot_create: |path| format!("Impossibile creare {path}"),
};
