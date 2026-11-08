//! Folder list in the settings (`folder_view.rs`): check marks, description for the AI, *Recognise by*, search.

pub struct Texts {
    // Catch-all folder
    pub fallback_top: &'static str,
    pub fallback_hint: &'static str,
    pub fallback_missing: fn(&str) -> String,
    pub fallback_unused: fn(&str) -> String,

    // Explanation, search and counters above the overview
    pub intro: &'static str,
    pub migrated: fn(&str) -> String,
    pub search_hint: &'static str,
    pub expand_all: &'static str,
    pub collapse_all: &'static str,
    pub scanning: &'static str,
    pub count_folders: fn(usize, usize) -> String,
    pub count_with_words: fn(usize) -> String,
    pub legend: &'static str,
    pub no_folders: &'static str,
    pub no_match: &'static str,

    // Rows
    pub collapse_subfolders: &'static str,
    pub show_subfolders: &'static str,
    pub parent_unused: &'static str,
    /// Fields of an unused folder.
    pub folder_unused: &'static str,
    pub use_hint: &'static str,
    pub years_tip: &'static str,
    pub subfolders: &'static str,
    pub years_badge: &'static str,
    pub note_hint: &'static str,
    pub series_hint: &'static str,

    // Entries without a folder
    pub leftovers: fn(usize) -> String,
    pub leftovers_hint: &'static str,
    pub remove_note: &'static str,
    pub orphan_words: &'static str,
    pub remove_words: &'static str,
    pub orphan_unused: &'static str,
    pub orphan_collection: &'static str,
    pub remove_entry: &'static str,

    // Keywords in words (on hover) and help for *Recognise by*
    pub quote: fn(&str) -> String,
    pub word_and: &'static str,
    pub word_or: &'static str,
    pub no_own_words: &'static str,
    pub matches: fn(&str) -> String,
    pub not_counting: fn(&str) -> String,
    pub words_help: &'static str,

    // Column headers
    pub col_folder: &'static str,
    pub col_description: &'static str,
    pub col_words: &'static str,
}

pub fn t() -> &'static Texts {
    super::pick(&DE, &EN, &FR, &ES, &IT)
}

pub static DE: Texts = Texts {
    fallback_top: "(oberste Ebene der Ablage)",
    fallback_hint: "Hierhin kommt, was keinem Ordner sicher zugeordnet werden kann",
    fallback_missing: |f| format!("Den Auffangordner „{f}“ gibt es nicht"),
    fallback_unused: |f| format!("Der Auffangordner „{f}“ wird nicht verwendet"),

    intro: "Haken: Scrinium verwendet den Ordner – ohne Haken bleibt er samt Unterordnern außen vor. Beschreibe \
            kurz, was hineingehört: Danach wählt die KI den Ordner Ebene für Ebene, die oberste Ebene ist am \
            wichtigsten. Jahresordner wählt Scrinium selbst. Unter „Erkennen an“ stehen nur eindeutige Merkmale \
            wie Bank- oder Versicherungsnamen und Nummern: Sie entscheiden ohne KI.",
    migrated: |list| format!("Frühere Sammelordner übernommen – ihre Unterordner sind jetzt nicht verwendet: {list}"),
    search_hint: "Ordner, Beschreibung oder Stichwort suchen",
    expand_all: "Alle aufklappen",
    collapse_all: "Alle zuklappen",
    scanning: "Ordner werden eingelesen …",
    count_folders: |total, described| format!("{total} Ordner, {described} beschrieben"),
    count_with_words: |n| format!("{n} mit Stichwörtern"),
    legend: "Erkennen an: Komma = eines davon · „ & “ = alle zusammen · „*“ = Wortanfang · \
             „-“ davor = Gegenwort (hebt die Wörter des Ordners auf) · entscheidet vor der KI",
    no_folders: "Keine Ordner gefunden – Ablageordner prüfen.",
    no_match: "Kein Ordner passt zur Suche.",

    collapse_subfolders: "Unterordner zuklappen",
    show_subfolders: "Unterordner zeigen",
    parent_unused: "Ein Oberordner wird nicht verwendet",
    folder_unused: "Der Ordner wird nicht verwendet – Haken links setzen",
    use_hint: "Verwenden: Scrinium zeigt und schlägt diesen Ordner vor. Ohne Haken bleibt er samt Unterordnern \
               außen vor.",
    years_tip: "mit Jahresordnern – das Jahr wählt Scrinium selbst",
    subfolders: "Unterordner",
    years_badge: "+ Jahre",
    note_hint: "was hineingehört",
    series_hint: "Jahresreihe – {year} steht für das Jahr",

    leftovers: |n| format!("Einträge ohne Ordner ({n})"),
    leftovers_hint: "Diese Ordner gibt es in der Ablage nicht (oder sie liegen tiefer als eingelesen).",
    remove_note: "Beschreibung entfernen",
    orphan_words: "Stichwörter",
    remove_words: "Stichwörter entfernen",
    orphan_unused: "nicht verwendet",
    orphan_collection: "Sammelordner",
    remove_entry: "Eintrag entfernen",

    quote: |s| format!("„{s}“"),
    word_and: " und ",
    word_or: " oder ",
    no_own_words: "Keine eigenen Stichwörter",
    matches: |words| format!("Trifft bei {words}"),
    not_counting: |words| format!(" – Stichwörter zählen nicht bei {words}"),
    words_help: "Erkennen an: eindeutige Merkmale, an denen Scrinium den Ordner ohne KI erkennt – \
                 Bank- oder Versicherungsnamen, Konto- und Vertragsnummern. Komma = eines davon genügt · \
                 „ & “ (mit Leerzeichen) = alle zusammen · „*“ am Ende = auch längere Wörter (Versicherungs*) · \
                 „-“ davor = Gegenwort: Kommt es vor, zählen die Wörter dieses Ordners und seiner Unterordner \
                 nicht. Ganze Wörter, ohne Rücksicht auf Groß/Klein und Umlaute. Trifft auf einer Ebene \
                 genau ein Ordner, nimmt Scrinium ihn; sonst entscheidet die KI nach den Beschreibungen. \
                 Wörter eines Unterordners zählen auch für den Oberordner.",

    col_folder: "Verwendet · Ordner",
    col_description: "Beschreibung für die KI",
    col_words: "Erkennen an",
};

pub static EN: Texts = Texts {
    fallback_top: "(top level of the archive)",
    fallback_hint: "Anything that cannot be assigned to a folder with confidence goes here",
    fallback_missing: |f| format!("The fallback folder “{f}” does not exist"),
    fallback_unused: |f| format!("The fallback folder “{f}” is not used"),

    intro: "Tick: Scrinium uses the folder – without a tick it is left out, along with its subfolders. Briefly \
            describe what belongs in it: the AI uses this to choose the folder level by level, and the top level \
            matters most. Scrinium picks year folders itself. Under “Recognised by”, enter only unique features \
            such as bank or insurer names and numbers: they decide without the AI.",
    migrated: |list| format!("Earlier collection folders taken over – their subfolders are now not used: {list}"),
    search_hint: "Search folders, descriptions or keywords",
    expand_all: "Expand all",
    collapse_all: "Collapse all",
    scanning: "Reading folders …",
    count_folders: |total, described| {
        let folders = if total == 1 { "folder" } else { "folders" };
        format!("{total} {folders}, {described} described")
    },
    count_with_words: |n| format!("{n} with keywords"),
    legend: "Recognised by: comma = any one · “ & ” = all together · “*” = word start · \
             “-” in front = negative word (cancels the folder's words) · decides before the AI",
    no_folders: "No folders found – check the archive folder.",
    no_match: "No folder matches the search.",

    collapse_subfolders: "Collapse subfolders",
    show_subfolders: "Show subfolders",
    parent_unused: "A parent folder is not used",
    folder_unused: "The folder is not used – tick the box on the left",
    use_hint: "Use: Scrinium shows and suggests this folder. Without a tick it is left out, along with its \
               subfolders.",
    years_tip: "with year folders – Scrinium picks the year itself",
    subfolders: "Subfolders",
    years_badge: "+ years",
    note_hint: "what belongs here",
    series_hint: "Year series – {year} stands for the year",

    leftovers: |n| format!("Entries without a folder ({n})"),
    leftovers_hint: "These folders do not exist in the archive (or lie deeper than was read).",
    remove_note: "Remove description",
    orphan_words: "keywords",
    remove_words: "Remove keywords",
    orphan_unused: "not used",
    orphan_collection: "collection folder",
    remove_entry: "Remove entry",

    quote: |s| format!("“{s}”"),
    word_and: " and ",
    word_or: " or ",
    no_own_words: "No keywords of its own",
    matches: |words| format!("Matches {words}"),
    not_counting: |words| format!(" – keywords do not count with {words}"),
    words_help: "Recognised by: unique features by which Scrinium recognises the folder without the AI – \
                 bank or insurer names, account and contract numbers. Comma = any one is enough · \
                 “ & ” (with spaces) = all together · “*” at the end = longer words too (insur*) · \
                 “-” in front = negative word: if it occurs, the words of this folder and its subfolders do not \
                 count. Whole words, ignoring upper/lower case and umlauts. If exactly one folder on a level \
                 matches, Scrinium takes it; otherwise the AI decides based on the descriptions. \
                 Words of a subfolder also count for the parent folder.",

    col_folder: "Used · Folder",
    col_description: "Description for the AI",
    col_words: "Recognised by",
};

pub static FR: Texts = Texts {
    fallback_top: "(niveau supérieur de l'archive)",
    fallback_hint: "Ce qui ne peut être attribué avec certitude à aucun dossier arrive ici",
    fallback_missing: |f| format!("Le dossier de repli « {f} » n'existe pas"),
    fallback_unused: |f| format!("Le dossier de repli « {f} » n'est pas utilisé"),

    intro: "Case cochée : Scrinium utilise le dossier – sans coche, il est ignoré avec ses sous-dossiers. Décrivez \
            brièvement ce qui y va : l'IA s'en sert pour choisir le dossier niveau par niveau, le niveau supérieur \
            étant le plus important. Scrinium choisit lui-même les dossiers annuels. Sous « Reconnu à », n'indiquez \
            que des critères sans ambiguïté, comme des noms de banques ou d'assureurs et des numéros : ils décident \
            sans l'IA.",
    migrated: |list| {
        format!("Anciens dossiers de regroupement repris – leurs sous-dossiers ne sont plus utilisés : {list}")
    },
    search_hint: "Rechercher un dossier, une description ou un mot-clé",
    expand_all: "Tout déplier",
    collapse_all: "Tout replier",
    scanning: "Lecture des dossiers …",
    count_folders: |total, described| {
        let (folders, done) =
            (if total <= 1 { "dossier" } else { "dossiers" }, if described <= 1 { "décrit" } else { "décrits" });
        format!("{total} {folders}, {described} {done}")
    },
    count_with_words: |n| format!("{n} avec mots-clés"),
    legend: "Reconnu à : virgule = l'un d'eux · « & » = tous ensemble · «*» = début de mot · \
             «-» devant = mot négatif (annule les mots du dossier) · décide avant l'IA",
    no_folders: "Aucun dossier trouvé – vérifiez le dossier d'archive.",
    no_match: "Aucun dossier ne correspond à la recherche.",

    collapse_subfolders: "Replier les sous-dossiers",
    show_subfolders: "Afficher les sous-dossiers",
    parent_unused: "Un dossier parent n'est pas utilisé",
    folder_unused: "Le dossier n'est pas utilisé – cochez la case à gauche",
    use_hint: "Utiliser : Scrinium affiche et propose ce dossier. Sans coche, il est ignoré avec ses sous-dossiers.",
    years_tip: "avec dossiers annuels – Scrinium choisit lui-même l'année",
    subfolders: "Sous-dossiers",
    years_badge: "+ années",
    note_hint: "ce qui va dedans",
    series_hint: "Série annuelle – {year} représente l'année",

    leftovers: |n| format!("Entrées sans dossier ({n})"),
    leftovers_hint: "Ces dossiers n'existent pas dans l'archive (ou sont plus profonds que la lecture).",
    remove_note: "Retirer la description",
    orphan_words: "mots-clés",
    remove_words: "Retirer les mots-clés",
    orphan_unused: "non utilisé",
    orphan_collection: "dossier de regroupement",
    remove_entry: "Retirer l'entrée",

    quote: |s| format!("« {s} »"),
    word_and: " et ",
    word_or: " ou ",
    no_own_words: "Pas de mots-clés propres",
    matches: |words| format!("Correspond à {words}"),
    not_counting: |words| format!(" – les mots-clés ne comptent pas en présence de {words}"),
    words_help: "Reconnu à : critères sans ambiguïté grâce auxquels Scrinium reconnaît le dossier sans l'IA – \
                 noms de banques ou d'assureurs, numéros de compte et de contrat. Virgule = l'un d'eux suffit · \
                 « & » (avec espaces) = tous ensemble · «*» à la fin = aussi des mots plus longs (assur*) · \
                 «-» devant = mot négatif : s'il apparaît, les mots de ce dossier et de ses sous-dossiers ne \
                 comptent pas. Mots entiers, sans distinction de majuscules ni de trémas (ä = a). Si exactement un \
                 dossier correspond à un niveau, Scrinium le prend ; sinon l'IA décide d'après les descriptions. \
                 Les mots d'un sous-dossier comptent aussi pour le dossier parent.",

    col_folder: "Utilisé · Dossier",
    col_description: "Description pour l'IA",
    col_words: "Reconnu à",
};

pub static ES: Texts = Texts {
    fallback_top: "(nivel superior del archivo)",
    fallback_hint: "Aquí va lo que no se puede asignar con seguridad a ninguna carpeta",
    fallback_missing: |f| format!("La carpeta de reserva «{f}» no existe"),
    fallback_unused: |f| format!("La carpeta de reserva «{f}» no se usa"),

    intro: "Casilla marcada: Scrinium usa la carpeta – sin marcar, queda fuera junto con sus subcarpetas. Describe \
            brevemente qué va dentro: con eso la IA elige la carpeta nivel por nivel; el nivel superior es el más \
            importante. Las carpetas anuales las elige Scrinium. En «Se reconoce por» pon solo rasgos inequívocos, \
            como nombres de bancos o aseguradoras y números: deciden sin la IA.",
    migrated: |list| format!("Antiguas carpetas de agrupación adoptadas – sus subcarpetas ya no se usan: {list}"),
    search_hint: "Buscar carpeta, descripción o palabra clave",
    expand_all: "Desplegar todo",
    collapse_all: "Plegar todo",
    scanning: "Leyendo carpetas …",
    count_folders: |total, described| {
        let (folders, done) =
            (if total == 1 { "carpeta" } else { "carpetas" }, if described == 1 { "descrita" } else { "descritas" });
        format!("{total} {folders}, {described} {done}")
    },
    count_with_words: |n| format!("{n} con palabras clave"),
    legend: "Se reconoce por: coma = uno de ellos · « & » = todos juntos · «*» = inicio de palabra · \
             «-» delante = palabra negativa (anula las palabras de la carpeta) · decide antes que la IA",
    no_folders: "No se encontraron carpetas – revisa la carpeta del archivo.",
    no_match: "Ninguna carpeta coincide con la búsqueda.",

    collapse_subfolders: "Plegar subcarpetas",
    show_subfolders: "Mostrar subcarpetas",
    parent_unused: "Una carpeta superior no se usa",
    folder_unused: "La carpeta no se usa – marque la casilla de la izquierda",
    use_hint: "Usar: Scrinium muestra y propone esta carpeta. Sin marcar, queda fuera junto con sus subcarpetas.",
    years_tip: "con carpetas anuales – Scrinium elige el año por sí solo",
    subfolders: "Subcarpetas",
    years_badge: "+ años",
    note_hint: "qué va dentro",
    series_hint: "Serie anual – {year} representa el año",

    leftovers: |n| format!("Entradas sin carpeta ({n})"),
    leftovers_hint: "Estas carpetas no existen en el archivo (o están a más profundidad de la leída).",
    remove_note: "Quitar descripción",
    orphan_words: "palabras clave",
    remove_words: "Quitar palabras clave",
    orphan_unused: "sin usar",
    orphan_collection: "carpeta de agrupación",
    remove_entry: "Quitar entrada",

    quote: |s| format!("«{s}»"),
    word_and: " y ",
    word_or: " o ",
    no_own_words: "Sin palabras clave propias",
    matches: |words| format!("Coincide con {words}"),
    not_counting: |words| format!(" – las palabras clave no cuentan en presencia de {words}"),
    words_help: "Se reconoce por: rasgos inequívocos con los que Scrinium reconoce la carpeta sin la IA – \
                 nombres de bancos o aseguradoras, números de cuenta y de contrato. Coma = basta uno de ellos · \
                 « & » (con espacios) = todos juntos · «*» al final = también palabras más largas (asegur*) · \
                 «-» delante = palabra negativa: si aparece, no cuentan las palabras de esta carpeta ni de sus \
                 subcarpetas. Palabras completas, sin distinguir mayúsculas ni diéresis (ä = a). Si en un nivel \
                 coincide exactamente una carpeta, Scrinium la toma; si no, la IA decide según las descripciones. \
                 Las palabras de una subcarpeta cuentan también para la carpeta superior.",

    col_folder: "Usada · Carpeta",
    col_description: "Descripción para la IA",
    col_words: "Se reconoce por",
};

pub static IT: Texts = Texts {
    fallback_top: "(livello superiore dell'archivio)",
    fallback_hint: "Qui finisce ciò che non si può assegnare con sicurezza a nessuna cartella",
    fallback_missing: |f| format!("La cartella di riserva «{f}» non esiste"),
    fallback_unused: |f| format!("La cartella di riserva «{f}» non viene usata"),

    intro: "Spunta: Scrinium usa la cartella – senza spunta resta esclusa insieme alle sottocartelle. Descrivi in \
            breve cosa ci va dentro: in base a questo l'IA sceglie la cartella livello per livello, e il livello \
            più alto conta di più. Le cartelle annuali le sceglie Scrinium. In «Riconosciuto da» metti solo segni \
            inequivocabili come nomi di banche o assicurazioni e numeri: decidono senza IA.",
    migrated: |list| format!("Ex cartelle di raccolta riprese – le loro sottocartelle ora non sono usate: {list}"),
    search_hint: "Cerca cartella, descrizione o parola chiave",
    expand_all: "Espandi tutto",
    collapse_all: "Comprimi tutto",
    scanning: "Lettura delle cartelle …",
    count_folders: |total, described| {
        let (folders, done) =
            (if total == 1 { "cartella" } else { "cartelle" }, if described == 1 { "descritta" } else { "descritte" });
        format!("{total} {folders}, {described} {done}")
    },
    count_with_words: |n| format!("{n} con parole chiave"),
    legend: "Riconosciuto da: virgola = uno di essi · « & » = tutti insieme · «*» = inizio parola · \
             «-» davanti = parola negativa (annulla le parole della cartella) · decide prima dell'IA",
    no_folders: "Nessuna cartella trovata – controlla la cartella dell'archivio.",
    no_match: "Nessuna cartella corrisponde alla ricerca.",

    collapse_subfolders: "Comprimi sottocartelle",
    show_subfolders: "Mostra sottocartelle",
    parent_unused: "Una cartella superiore non viene usata",
    folder_unused: "La cartella non viene usata – spunta la casella a sinistra",
    use_hint: "Usa: Scrinium mostra e propone questa cartella. Senza spunta resta esclusa insieme alle \
               sottocartelle.",
    years_tip: "con cartelle annuali – l'anno lo sceglie Scrinium",
    subfolders: "Sottocartelle",
    years_badge: "+ anni",
    note_hint: "cosa ci va dentro",
    series_hint: "Serie annuale – {year} indica l'anno",

    leftovers: |n| format!("Voci senza cartella ({n})"),
    leftovers_hint: "Queste cartelle non esistono nell'archivio (o sono più in profondità di quanto letto).",
    remove_note: "Rimuovi descrizione",
    orphan_words: "parole chiave",
    remove_words: "Rimuovi parole chiave",
    orphan_unused: "non usata",
    orphan_collection: "cartella di raccolta",
    remove_entry: "Rimuovi voce",

    quote: |s| format!("«{s}»"),
    word_and: " e ",
    word_or: " o ",
    no_own_words: "Nessuna parola chiave propria",
    matches: |words| format!("Corrisponde a {words}"),
    not_counting: |words| format!(" – le parole chiave non contano in presenza di {words}"),
    words_help: "Riconosciuto da: segni inequivocabili con cui Scrinium riconosce la cartella senza IA – \
                 nomi di banche o assicurazioni, numeri di conto e di contratto. Virgola = ne basta uno · \
                 « & » (con spazi) = tutti insieme · «*» alla fine = anche parole più lunghe (assicura*) · \
                 «-» davanti = parola negativa: se compare, le parole di questa cartella e delle sue sottocartelle \
                 non contano. Parole intere, senza distinguere maiuscole/minuscole e dieresi (ä = a). Se su un \
                 livello corrisponde esattamente una cartella, Scrinium la prende; altrimenti decide l'IA in base \
                 alle descrizioni. Le parole di una sottocartella contano anche per la cartella superiore.",

    col_folder: "Usata · Cartella",
    col_description: "Descrizione per l'IA",
    col_words: "Riconosciuto da",
};
