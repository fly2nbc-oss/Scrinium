//! AI and models (`llm_ui.rs`, `local_llm.rs`, `download.rs`, `llm.rs`): graphics card, models, downloads, server,
//! messages.

pub struct Texts {
    // Status of the built-in AI; `embedded_on` is a prefix before the device, the status line cuts it off.
    pub cpu_no_gpu: &'static str,
    pub embedded_on: &'static str,
    pub model_ready: fn(&str) -> String,
    pub download_of: fn(&str, &str) -> String,

    // Graphics card
    pub runs_locally: &'static str,
    pub graphics_card: &'static str,
    pub check_again: &'static str,
    /// *Check again* disabled.
    pub gpu_checking: &'static str,
    pub searching_gpus: &'static str,
    pub device_line: fn(&str, &str) -> String,
    pub device_free: fn(&str) -> String,
    pub in_use: &'static str,
    pub no_gpu_hint: &'static str,
    pub recommendation: fn(&str) -> String,

    // Models
    pub model: &'static str,
    pub own_model_file: &'static str,
    pub auto_for_hardware: fn(&str) -> String,
    pub auto_matching: &'static str,
    pub recommended: &'static str,
    pub cancel: &'static str,
    pub installed: &'static str,
    pub delete: &'static str,
    pub delete_hover: &'static str,
    pub download: &'static str,
    /// Download disabled.
    pub download_running: &'static str,
    pub source_and_location: fn(&str) -> String,
    pub delete_title: fn(&str) -> String,
    pub delete_text: fn(&str) -> String,

    // Advanced
    pub advanced: &'static str,
    pub llama_cpp_hint: &'static str,
    pub gpu_layers: &'static str,
    pub layers_all: &'static str,
    pub layers_none: &'static str,
    pub layers_some: fn(i32) -> String,
    pub context_length: &'static str,
    pub context_hint: &'static str,
    pub max_chars: &'static str,
    pub own_gguf: &'static str,
    pub own_gguf_hint: &'static str,
    pub choose_model: &'static str,
    pub image_projector: &'static str,
    pub projector_hint: &'static str,
    pub choose_projector: &'static str,

    // Notes on the models in the catalogue
    pub note_fast: &'static str,
    pub note_balanced: &'static str,
    pub note_best: &'static str,

    // Compute devices and recommendation
    pub llama_start_failed: fn(&str) -> String,
    pub integrated_graphics: &'static str,
    pub processor: &'static str,
    pub memory_shared: &'static str,
    pub memory_vram: &'static str,
    pub reason_no_gpu: &'static str,
    /// Graphics card, memory, memory type, model, does it fit entirely?
    pub reason_gpu: fn(&str, &str, &str, &str, bool) -> String,

    // Model files
    pub model_file_missing: fn(&str) -> String,
    pub projector_missing: fn(&str) -> String,
    pub unknown_model: fn(&str) -> String,
    pub not_downloaded: fn(&str) -> String,
    pub no_model_yet: fn(&str, &str) -> String,
    pub download_corrupt: fn(&str) -> String,
    pub cannot_delete: fn(&str) -> String,

    // Loading and inference
    pub prompt_too_long: fn(usize, usize) -> String,
    pub model_load_failed: fn(&str, &str) -> String,
    pub gpu_load_failed: fn(&str) -> String,
    pub projector_path_invalid: &'static str,
    pub projector_not_loaded: fn(&str) -> String,
    pub device_via: fn(&str, &str) -> String,
    pub err_json_grammar: fn(&str) -> String,
    pub err_image: fn(&str) -> String,
    pub err_image_tokens: fn(&str) -> String,
    pub err_image_eval: fn(&str) -> String,
    pub answer_truncated: fn(usize) -> String,
    pub err_chat_template: fn(&str) -> String,
    pub err_context: fn(u32, &str) -> String,
    pub err_eval: fn(&str) -> String,
    pub cancelled: &'static str,
    /// Input tokens, seconds, output tokens, seconds.
    pub gen_stats: fn(usize, f32, usize, f32) -> String,

    // Downloads
    pub progress_of: fn(&str, &str, &str) -> String,
    pub progress_done: fn(&str, &str) -> String,
    pub decimal_separator: &'static str,
    pub unit_gb: &'static str,
    pub unit_mb: &'static str,
    pub cannot_create: fn(&str) -> String,
    pub download_http_failed: fn(&str, u16) -> String,
    pub download_failed: fn(&str, &str) -> String,
    pub cannot_write: fn(&str) -> String,
    pub download_cancelled: &'static str,
    pub download_interrupted: fn(&str) -> String,
    pub download_incomplete: fn(&str, &str) -> String,
    pub cannot_rename: fn(&str) -> String,

    // AI server (LM Studio, Ollama …)
    pub server_unreachable_at: fn(&str, &str) -> String,
    pub server_no_model: &'static str,
    pub server_status: fn(u16, &str) -> String,
    pub server_unreachable: fn(&str) -> String,
    pub server_unexpected: &'static str,
    pub server_rejected: fn(&str) -> String,
    pub answer_no_json: &'static str,
    pub answer_incomplete_json: &'static str,
    pub answer_invalid_json: &'static str,
    pub answer_bad_format: &'static str,
}

pub fn t() -> &'static Texts {
    super::pick(&DE, &EN, &FR, &ES, &IT)
}

pub static DE: Texts = Texts {
    cpu_no_gpu: "Prozessor (keine GPU)",
    embedded_on: "Eingebaute KI auf ",
    model_ready: |name| format!("{name} ist heruntergeladen und einsatzbereit"),
    download_of: |name, e| format!("Download von {name}: {e}"),

    runs_locally: "Die KI läuft auf diesem Rechner, nichts verlässt ihn. Dafür genügt der normale Grafiktreiber.",
    graphics_card: "Grafikkarte",
    check_again: "Erneut prüfen",
    gpu_checking: "Die Prüfung läuft gerade",
    searching_gpus: "Suche Grafikkarten …",
    device_line: |kind, summary| format!("{kind}: {summary}"),
    device_free: |mem| format!(", frei {mem}"),
    in_use: "wird verwendet",
    no_gpu_hint: "Keine Grafikkarte nutzbar. Aktuellen Grafiktreiber installieren (AMD Adrenalin bzw. Mesa RADV unter \
                  Linux) und erneut prüfen – sonst rechnet die KI auf dem Prozessor (langsamer).",
    recommendation: |reason| format!("Empfehlung: {reason}"),

    model: "Modell",
    own_model_file: "Eigene Modelldatei ist gesetzt (unter „Erweitert“) und hat Vorrang.",
    auto_for_hardware: |name| format!("Automatisch (für diese Hardware: {name})"),
    auto_matching: "Automatisch passend zur Hardware",
    recommended: "empfohlen",
    cancel: "Abbrechen",
    installed: "installiert",
    delete: "Löschen",
    delete_hover: "Modelldateien entfernen",
    download: "Herunterladen",
    download_running: "Es läuft schon ein Download",
    source_and_location: |dir| {
        format!("Quelle: Hugging Face (Qwen). Abgebrochene Downloads werden fortgesetzt. Speicherort: {dir}")
    },
    delete_title: |name| format!("{name} löschen?"),
    delete_text: |size| {
        format!(
            "Die Modelldateien ({size}) werden entfernt. Um das Modell wieder zu verwenden, muss es neu \
             heruntergeladen werden."
        )
    },

    advanced: "Erweitert",
    llama_cpp_hint: "Die KI läuft mit llama.cpp. Grafikkarten von AMD, NVIDIA und Intel werden über Vulkan genutzt – \
                     ROCm oder CUDA sind nicht nötig.",
    gpu_layers: "GPU-Schichten",
    layers_all: "alle auf der GPU",
    layers_none: "nur Prozessor",
    layers_some: |n| {
        if n == 1 {
            "1 Schicht auf der GPU, Rest auf dem Prozessor".to_string()
        } else {
            format!("{n} Schichten auf der GPU, Rest auf dem Prozessor")
        }
    },
    context_length: "Kontextlänge",
    context_hint: "Token für Bild, Text und Antwort",
    max_chars: "Max. Textzeichen",
    own_gguf: "Eigene GGUF-Datei",
    own_gguf_hint: "leer = Modell aus der Liste",
    choose_model: "Modell wählen",
    image_projector: "Bild-Projektor",
    projector_hint: "mmproj-….gguf (für Bilder)",
    choose_projector: "Bild-Projektor wählen",

    note_fast: "schnell, auch ohne GPU nutzbar",
    note_balanced: "ausgewogen, ab ca. 6 GB Grafikspeicher",
    note_best: "beste Qualität, ab ca. 10 GB Grafikspeicher",

    llama_start_failed: |e| format!("llama.cpp konnte nicht gestartet werden: {e}"),
    integrated_graphics: "integrierte Grafik",
    processor: "Prozessor",
    memory_shared: "geteilter Speicher",
    memory_vram: "VRAM",
    reason_no_gpu: "Keine nutzbare GPU gefunden – das kleine Modell läuft auch auf dem Prozessor.",
    reason_gpu: |gpu, mem, label, model, fits| {
        let fit = if fits { "passt vollständig hinein" } else { "läuft teilweise auf dem Prozessor" };
        format!("{gpu} mit {mem} {label} → {model} {fit}.")
    },

    model_file_missing: |path| format!("Modelldatei {path} nicht gefunden"),
    projector_missing: |path| format!("Bild-Projektor {path} nicht gefunden"),
    unknown_model: |id| format!("Unbekanntes Modell „{id}“"),
    not_downloaded: |name| {
        format!(
            "{name} ist noch nicht heruntergeladen – bitte unter Einstellungen › KI › Modell und Grafikkarte \
             herunterladen."
        )
    },
    no_model_yet: |name, size| {
        format!(
            "Noch kein KI-Modell heruntergeladen. Empfohlen für diesen Rechner: {name} ({size}) – bitte unter \
             Einstellungen › KI › Modell und Grafikkarte herunterladen."
        )
    },
    download_corrupt: |file| format!("{file} ist beschädigt angekommen – bitte erneut herunterladen"),
    cannot_delete: |path| format!("{path} nicht löschbar"),

    prompt_too_long: |tokens, budget| {
        format!("Eingabe zu lang ({tokens} Token, Kontext {budget}) – Kontextlänge erhöhen oder weniger Text senden")
    },
    model_load_failed: |path, e| format!("Modell {path} konnte nicht geladen werden: {e}"),
    gpu_load_failed: |e| format!("KI läuft auf dem Prozessor, GPU-Laden fehlgeschlagen: {e}"),
    projector_path_invalid: "Pfad des Bild-Projektors ist kein gültiger Text",
    projector_not_loaded: |e| format!("Bild-Projektor nicht geladen, KI liest nur den Text: {e}"),
    device_via: |name, backend| format!("{name} über {backend}"),
    err_json_grammar: |e| format!("JSON-Grammatik: {e}"),
    err_image: |e| format!("Bild: {e}"),
    err_image_tokens: |e| format!("Bild-Tokenisierung: {e}"),
    err_image_eval: |e| format!("Bildauswertung: {e}"),
    answer_truncated: |n| format!("Die Antwort der KI wurde nach {n} Token abgeschnitten"),
    err_chat_template: |e| format!("Chat-Vorlage: {e}"),
    err_context: |n, e| format!("KI-Kontext ({n} Token): {e}"),
    err_eval: |e| format!("KI-Auswertung: {e}"),
    cancelled: "Abgebrochen",
    gen_stats: |input, input_s, output, output_s| {
        format!("Eingabe {input} Token in {input_s:.1} s, Antwort {output} Token in {output_s:.1} s").replace('.', ",")
    },

    progress_of: |label, done, total| format!("{label}: {done} von {total}"),
    progress_done: |label, done| format!("{label}: {done}"),
    decimal_separator: ",",
    unit_gb: "GB",
    unit_mb: "MB",
    cannot_create: |dir| format!("{dir} nicht anlegbar"),
    download_http_failed: |url, code| format!("Download von {url} fehlgeschlagen (HTTP {code})"),
    download_failed: |url, e| format!("Download von {url} fehlgeschlagen: {e}"),
    cannot_write: |path| format!("{path} nicht schreibbar"),
    download_cancelled: "Download abgebrochen",
    download_interrupted: |e| format!("Download unterbrochen: {e}"),
    download_incomplete: |done, total| {
        format!("Download unvollständig ({done} von {total}) – erneut versuchen setzt ihn fort")
    },
    cannot_rename: |path| format!("{path} nicht umbenennbar"),

    server_unreachable_at: |url, e| format!("KI-Server nicht erreichbar ({url}): {e}"),
    server_no_model: "Im KI-Server ist kein Modell geladen",
    server_status: |code, text| format!("KI-Server antwortet mit {code}: {text}"),
    server_unreachable: |e| format!("KI-Server nicht erreichbar: {e}"),
    server_unexpected: "Unerwartete Antwort des KI-Servers",
    server_rejected: |text| format!("KI-Server lehnt die Anfrage ab: {text}"),
    answer_no_json: "KI-Antwort enthält kein JSON",
    answer_incomplete_json: "KI-Antwort enthält kein vollständiges JSON",
    answer_invalid_json: "KI-Antwort ist kein gültiges JSON",
    answer_bad_format: "KI-Antwort hat ein unerwartetes Format",
};

pub static EN: Texts = Texts {
    cpu_no_gpu: "Processor (no GPU)",
    embedded_on: "Built-in AI on ",
    model_ready: |name| format!("{name} is downloaded and ready to use"),
    download_of: |name, e| format!("Download of {name}: {e}"),

    runs_locally: "The AI runs on this computer; nothing leaves it. The regular graphics driver is all it needs.",
    graphics_card: "Graphics card",
    check_again: "Check again",
    gpu_checking: "A check is in progress",
    searching_gpus: "Looking for graphics cards …",
    device_line: |kind, summary| format!("{kind}: {summary}"),
    device_free: |mem| format!(", {mem} free"),
    in_use: "in use",
    no_gpu_hint: "No usable graphics card. Install a current graphics driver (AMD Adrenalin, or Mesa RADV on Linux) \
                  and check again – otherwise the AI runs on the processor (slower).",
    recommendation: |reason| format!("Recommendation: {reason}"),

    model: "Model",
    own_model_file: "A custom model file is set (under “Advanced”) and takes priority.",
    auto_for_hardware: |name| format!("Automatic (for this hardware: {name})"),
    auto_matching: "Automatic, to match the hardware",
    recommended: "recommended",
    cancel: "Cancel",
    installed: "installed",
    delete: "Delete",
    delete_hover: "Remove the model files",
    download: "Download",
    download_running: "Another download is running",
    source_and_location: |dir| {
        format!("Source: Hugging Face (Qwen). Interrupted downloads are resumed. Location: {dir}")
    },
    delete_title: |name| format!("Delete {name}?"),
    delete_text: |size| {
        format!("The model files ({size}) will be removed. To use the model again, you will have to download it again.")
    },

    advanced: "Advanced",
    llama_cpp_hint: "The AI runs with llama.cpp. Graphics cards from AMD, NVIDIA and Intel are used via Vulkan – ROCm \
                     or CUDA are not needed.",
    gpu_layers: "GPU layers",
    layers_all: "all on the GPU",
    layers_none: "processor only",
    layers_some: |n| {
        if n == 1 {
            "1 layer on the GPU, the rest on the processor".to_string()
        } else {
            format!("{n} layers on the GPU, the rest on the processor")
        }
    },
    context_length: "Context length",
    context_hint: "Tokens for image, text and answer",
    max_chars: "Max. text characters",
    own_gguf: "Custom GGUF file",
    own_gguf_hint: "empty = model from the list",
    choose_model: "Choose model",
    image_projector: "Image projector",
    projector_hint: "mmproj-….gguf (for images)",
    choose_projector: "Choose image projector",

    note_fast: "fast, also works without a GPU",
    note_balanced: "balanced, from about 6 GB of video memory",
    note_best: "best quality, from about 10 GB of video memory",

    llama_start_failed: |e| format!("llama.cpp could not be started: {e}"),
    integrated_graphics: "Integrated graphics",
    processor: "Processor",
    memory_shared: "shared memory",
    memory_vram: "VRAM",
    reason_no_gpu: "No usable GPU found – the small model also runs on the processor.",
    reason_gpu: |gpu, mem, label, model, fits| {
        let fit = if fits { "fits completely" } else { "runs partly on the processor" };
        format!("{gpu} with {mem} {label} → {model} {fit}.")
    },

    model_file_missing: |path| format!("Model file {path} not found"),
    projector_missing: |path| format!("Image projector {path} not found"),
    unknown_model: |id| format!("Unknown model “{id}”"),
    not_downloaded: |name| {
        format!("{name} is not downloaded yet – please download it under Settings › AI › Model and graphics card.")
    },
    no_model_yet: |name, size| {
        format!(
            "No AI model downloaded yet. Recommended for this computer: {name} ({size}) – please download it under \
             Settings › AI › Model and graphics card."
        )
    },
    download_corrupt: |file| format!("{file} arrived damaged – please download it again"),
    cannot_delete: |path| format!("{path} cannot be deleted"),

    prompt_too_long: |tokens, budget| {
        format!("Input too long ({tokens} tokens, context {budget}) – increase the context length or send less text")
    },
    model_load_failed: |path, e| format!("Model {path} could not be loaded: {e}"),
    gpu_load_failed: |e| format!("AI runs on the processor, loading onto the GPU failed: {e}"),
    projector_path_invalid: "The path of the image projector is not valid text",
    projector_not_loaded: |e| format!("Image projector not loaded, the AI reads only the text: {e}"),
    device_via: |name, backend| format!("{name} via {backend}"),
    err_json_grammar: |e| format!("JSON grammar: {e}"),
    err_image: |e| format!("Image: {e}"),
    err_image_tokens: |e| format!("Image tokenisation: {e}"),
    err_image_eval: |e| format!("Image evaluation: {e}"),
    answer_truncated: |n| format!("The AI's answer was cut off after {n} tokens"),
    err_chat_template: |e| format!("Chat template: {e}"),
    err_context: |n, e| format!("AI context ({n} tokens): {e}"),
    err_eval: |e| format!("AI evaluation: {e}"),
    cancelled: "Cancelled",
    gen_stats: |input, input_s, output, output_s| {
        format!("Input {input} tokens in {input_s:.1} s, answer {output} tokens in {output_s:.1} s")
    },

    progress_of: |label, done, total| format!("{label}: {done} of {total}"),
    progress_done: |label, done| format!("{label}: {done}"),
    decimal_separator: ".",
    unit_gb: "GB",
    unit_mb: "MB",
    cannot_create: |dir| format!("{dir} cannot be created"),
    download_http_failed: |url, code| format!("Download from {url} failed (HTTP {code})"),
    download_failed: |url, e| format!("Download from {url} failed: {e}"),
    cannot_write: |path| format!("{path} cannot be written"),
    download_cancelled: "Download cancelled",
    download_interrupted: |e| format!("Download interrupted: {e}"),
    download_incomplete: |done, total| format!("Download incomplete ({done} of {total}) – trying again resumes it"),
    cannot_rename: |path| format!("{path} cannot be renamed"),

    server_unreachable_at: |url, e| format!("AI server not reachable ({url}): {e}"),
    server_no_model: "No model is loaded in the AI server",
    server_status: |code, text| format!("AI server responds with {code}: {text}"),
    server_unreachable: |e| format!("AI server not reachable: {e}"),
    server_unexpected: "Unexpected answer from the AI server",
    server_rejected: |text| format!("AI server rejects the request: {text}"),
    answer_no_json: "AI answer contains no JSON",
    answer_incomplete_json: "AI answer contains no complete JSON",
    answer_invalid_json: "AI answer is not valid JSON",
    answer_bad_format: "AI answer has an unexpected format",
};

pub static FR: Texts = Texts {
    cpu_no_gpu: "Processeur (pas de GPU)",
    embedded_on: "IA intégrée sur ",
    model_ready: |name| format!("{name} est téléchargé et prêt à l'emploi"),
    download_of: |name, e| format!("Téléchargement de {name} : {e}"),

    runs_locally: "L'IA fonctionne sur cet ordinateur, rien n'en sort. Le pilote graphique habituel suffit.",
    graphics_card: "Carte graphique",
    check_again: "Vérifier à nouveau",
    gpu_checking: "Une vérification est en cours",
    searching_gpus: "Recherche des cartes graphiques …",
    device_line: |kind, summary| format!("{kind} : {summary}"),
    device_free: |mem| format!(", {mem} libres"),
    in_use: "en cours d'utilisation",
    no_gpu_hint: "Aucune carte graphique utilisable. Installez un pilote graphique récent (AMD Adrenalin, ou Mesa RADV \
                  sous Linux) et vérifiez à nouveau – sinon l'IA calcule sur le processeur (plus lent).",
    recommendation: |reason| format!("Recommandation : {reason}"),

    model: "Modèle",
    own_model_file: "Un fichier de modèle personnel est défini (sous « Avancé ») et a la priorité.",
    auto_for_hardware: |name| format!("Automatique (pour ce matériel : {name})"),
    auto_matching: "Automatique, selon le matériel",
    recommended: "recommandé",
    cancel: "Annuler",
    installed: "installé",
    delete: "Supprimer",
    delete_hover: "Supprimer les fichiers du modèle",
    download: "Télécharger",
    download_running: "Un autre téléchargement est en cours",
    source_and_location: |dir| {
        format!(
            "Source : Hugging Face (Qwen). Les téléchargements interrompus reprennent là où ils se sont arrêtés. \
             Emplacement : {dir}"
        )
    },
    delete_title: |name| format!("Supprimer {name} ?"),
    delete_text: |size| {
        format!(
            "Les fichiers du modèle ({size}) seront supprimés. Pour réutiliser le modèle, il faudra le télécharger \
             à nouveau."
        )
    },

    advanced: "Avancé",
    llama_cpp_hint: "L'IA fonctionne avec llama.cpp. Les cartes graphiques AMD, NVIDIA et Intel sont utilisées via \
                     Vulkan – ROCm ou CUDA ne sont pas nécessaires.",
    gpu_layers: "Couches GPU",
    layers_all: "toutes sur le GPU",
    layers_none: "processeur uniquement",
    layers_some: |n| {
        if n == 1 {
            "1 couche sur le GPU, le reste sur le processeur".to_string()
        } else {
            format!("{n} couches sur le GPU, le reste sur le processeur")
        }
    },
    context_length: "Longueur du contexte",
    context_hint: "Tokens pour l'image, le texte et la réponse",
    max_chars: "Caractères de texte max.",
    own_gguf: "Fichier GGUF personnel",
    own_gguf_hint: "vide = modèle de la liste",
    choose_model: "Choisir le modèle",
    image_projector: "Projecteur d'images",
    projector_hint: "mmproj-….gguf (pour les images)",
    choose_projector: "Choisir le projecteur d'images",

    note_fast: "rapide, utilisable aussi sans GPU",
    note_balanced: "équilibré, à partir d'env. 6 Go de mémoire graphique",
    note_best: "meilleure qualité, à partir d'env. 10 Go de mémoire graphique",

    llama_start_failed: |e| format!("Impossible de démarrer llama.cpp : {e}"),
    integrated_graphics: "Carte graphique intégrée",
    processor: "Processeur",
    memory_shared: "de mémoire partagée",
    memory_vram: "de VRAM",
    reason_no_gpu: "Aucun GPU utilisable trouvé – le petit modèle fonctionne aussi sur le processeur.",
    reason_gpu: |gpu, mem, label, model, fits| {
        let fit = if fits { "y tient entièrement" } else { "tourne en partie sur le processeur" };
        format!("{gpu} avec {mem} {label} → {model} {fit}.")
    },

    model_file_missing: |path| format!("Fichier de modèle {path} introuvable"),
    projector_missing: |path| format!("Projecteur d'images {path} introuvable"),
    unknown_model: |id| format!("Modèle inconnu « {id} »"),
    not_downloaded: |name| {
        format!("{name} n'est pas encore téléchargé – téléchargez-le dans Paramètres › IA › Modèle et carte graphique.")
    },
    no_model_yet: |name, size| {
        format!(
            "Aucun modèle d'IA téléchargé pour l'instant. Recommandé pour cet ordinateur : {name} ({size}) – \
             téléchargez-le dans Paramètres › IA › Modèle et carte graphique."
        )
    },
    download_corrupt: |file| format!("{file} est arrivé endommagé – téléchargez-le à nouveau"),
    cannot_delete: |path| format!("Impossible de supprimer {path}"),

    prompt_too_long: |tokens, budget| {
        format!(
            "Entrée trop longue ({tokens} tokens, contexte {budget}) – augmentez la longueur du contexte ou envoyez \
             moins de texte"
        )
    },
    model_load_failed: |path, e| format!("Impossible de charger le modèle {path} : {e}"),
    gpu_load_failed: |e| format!("L'IA tourne sur le processeur, le chargement sur le GPU a échoué : {e}"),
    projector_path_invalid: "Le chemin du projecteur d'images n'est pas un texte valide",
    projector_not_loaded: |e| format!("Projecteur d'images non chargé, l'IA ne lit que le texte : {e}"),
    device_via: |name, backend| format!("{name} via {backend}"),
    err_json_grammar: |e| format!("Grammaire JSON : {e}"),
    err_image: |e| format!("Image : {e}"),
    err_image_tokens: |e| format!("Tokenisation de l'image : {e}"),
    err_image_eval: |e| format!("Analyse de l'image : {e}"),
    answer_truncated: |n| format!("La réponse de l'IA a été coupée après {n} tokens"),
    err_chat_template: |e| format!("Modèle de chat : {e}"),
    err_context: |n, e| format!("Contexte de l'IA ({n} tokens) : {e}"),
    err_eval: |e| format!("Calcul de l'IA : {e}"),
    cancelled: "Annulé",
    gen_stats: |input, input_s, output, output_s| {
        format!("Entrée {input} tokens en {input_s:.1} s, réponse {output} tokens en {output_s:.1} s").replace('.', ",")
    },

    progress_of: |label, done, total| format!("{label} : {done} sur {total}"),
    progress_done: |label, done| format!("{label} : {done}"),
    decimal_separator: ",",
    unit_gb: "Go",
    unit_mb: "Mo",
    cannot_create: |dir| format!("Impossible de créer {dir}"),
    download_http_failed: |url, code| format!("Échec du téléchargement de {url} (HTTP {code})"),
    download_failed: |url, e| format!("Échec du téléchargement de {url} : {e}"),
    cannot_write: |path| format!("Impossible d'écrire {path}"),
    download_cancelled: "Téléchargement annulé",
    download_interrupted: |e| format!("Téléchargement interrompu : {e}"),
    download_incomplete: |done, total| {
        format!("Téléchargement incomplet ({done} sur {total}) – réessayez pour le reprendre")
    },
    cannot_rename: |path| format!("Impossible de renommer {path}"),

    server_unreachable_at: |url, e| format!("Serveur d'IA injoignable ({url}) : {e}"),
    server_no_model: "Aucun modèle n'est chargé dans le serveur d'IA",
    server_status: |code, text| format!("Le serveur d'IA répond {code} : {text}"),
    server_unreachable: |e| format!("Serveur d'IA injoignable : {e}"),
    server_unexpected: "Réponse inattendue du serveur d'IA",
    server_rejected: |text| format!("Le serveur d'IA refuse la requête : {text}"),
    answer_no_json: "La réponse de l'IA ne contient pas de JSON",
    answer_incomplete_json: "La réponse de l'IA ne contient pas de JSON complet",
    answer_invalid_json: "La réponse de l'IA n'est pas un JSON valide",
    answer_bad_format: "La réponse de l'IA a un format inattendu",
};

pub static ES: Texts = Texts {
    cpu_no_gpu: "Procesador (sin GPU)",
    embedded_on: "IA integrada en ",
    model_ready: |name| format!("{name} está descargado y listo para usar"),
    download_of: |name, e| format!("Descarga de {name}: {e}"),

    runs_locally: "La IA se ejecuta en este equipo, nada sale de él. Basta con el controlador gráfico normal.",
    graphics_card: "Tarjeta gráfica",
    check_again: "Volver a comprobar",
    gpu_checking: "Ya se está comprobando",
    searching_gpus: "Buscando tarjetas gráficas …",
    device_line: |kind, summary| format!("{kind}: {summary}"),
    device_free: |mem| format!(", {mem} libres"),
    in_use: "en uso",
    no_gpu_hint: "No hay ninguna tarjeta gráfica utilizable. Instala un controlador gráfico actual (AMD Adrenalin, o \
                  Mesa RADV en Linux) y vuelve a comprobar – si no, la IA calcula en el procesador (más lento).",
    recommendation: |reason| format!("Recomendación: {reason}"),

    model: "Modelo",
    own_model_file: "Hay un archivo de modelo propio (en «Avanzado») y tiene prioridad.",
    auto_for_hardware: |name| format!("Automático (para este hardware: {name})"),
    auto_matching: "Automático, según el hardware",
    recommended: "recomendado",
    cancel: "Cancelar",
    installed: "instalado",
    delete: "Eliminar",
    delete_hover: "Eliminar los archivos del modelo",
    download: "Descargar",
    download_running: "Ya hay una descarga en curso",
    source_and_location: |dir| {
        format!("Origen: Hugging Face (Qwen). Las descargas interrumpidas se reanudan. Ubicación: {dir}")
    },
    delete_title: |name| format!("¿Eliminar {name}?"),
    delete_text: |size| {
        format!(
            "Se eliminarán los archivos del modelo ({size}). Para volver a usar el modelo, tendrás que descargarlo \
             de nuevo."
        )
    },

    advanced: "Avanzado",
    llama_cpp_hint: "La IA funciona con llama.cpp. Las tarjetas gráficas de AMD, NVIDIA e Intel se usan mediante \
                     Vulkan – no hace falta ROCm ni CUDA.",
    gpu_layers: "Capas en la GPU",
    layers_all: "todas en la GPU",
    layers_none: "solo procesador",
    layers_some: |n| {
        if n == 1 {
            "1 capa en la GPU, el resto en el procesador".to_string()
        } else {
            format!("{n} capas en la GPU, el resto en el procesador")
        }
    },
    context_length: "Longitud de contexto",
    context_hint: "Tokens para imagen, texto y respuesta",
    max_chars: "Máx. caracteres de texto",
    own_gguf: "Archivo GGUF propio",
    own_gguf_hint: "vacío = modelo de la lista",
    choose_model: "Elegir modelo",
    image_projector: "Proyector de imagen",
    projector_hint: "mmproj-….gguf (para imágenes)",
    choose_projector: "Elegir proyector de imagen",

    note_fast: "rápido, también sin GPU",
    note_balanced: "equilibrado, desde unos 6 GB de memoria gráfica",
    note_best: "mejor calidad, desde unos 10 GB de memoria gráfica",

    llama_start_failed: |e| format!("No se pudo iniciar llama.cpp: {e}"),
    integrated_graphics: "Gráficos integrados",
    processor: "Procesador",
    memory_shared: "de memoria compartida",
    memory_vram: "de VRAM",
    reason_no_gpu: "No se encontró ninguna GPU utilizable – el modelo pequeño también funciona en el procesador.",
    reason_gpu: |gpu, mem, label, model, fits| {
        let fit = if fits { "cabe por completo" } else { "funciona en parte en el procesador" };
        format!("{gpu} con {mem} {label} → {model} {fit}.")
    },

    model_file_missing: |path| format!("No se encontró el archivo de modelo {path}"),
    projector_missing: |path| format!("No se encontró el proyector de imagen {path}"),
    unknown_model: |id| format!("Modelo desconocido «{id}»"),
    not_downloaded: |name| {
        format!("{name} aún no está descargado – descárgalo en Ajustes › IA › Modelo y tarjeta gráfica.")
    },
    no_model_yet: |name, size| {
        format!(
            "Aún no hay ningún modelo de IA descargado. Recomendado para este equipo: {name} ({size}) – descárgalo \
             en Ajustes › IA › Modelo y tarjeta gráfica."
        )
    },
    download_corrupt: |file| format!("{file} llegó dañado – descárgalo de nuevo"),
    cannot_delete: |path| format!("No se puede eliminar {path}"),

    prompt_too_long: |tokens, budget| {
        format!(
            "Entrada demasiado larga ({tokens} tokens, contexto {budget}) – aumenta la longitud de contexto o envía \
             menos texto"
        )
    },
    model_load_failed: |path, e| format!("No se pudo cargar el modelo {path}: {e}"),
    gpu_load_failed: |e| format!("La IA funciona en el procesador, falló la carga en la GPU: {e}"),
    projector_path_invalid: "La ruta del proyector de imagen no es un texto válido",
    projector_not_loaded: |e| format!("Proyector de imagen no cargado, la IA solo lee el texto: {e}"),
    device_via: |name, backend| format!("{name} mediante {backend}"),
    err_json_grammar: |e| format!("Gramática JSON: {e}"),
    err_image: |e| format!("Imagen: {e}"),
    err_image_tokens: |e| format!("Tokenización de la imagen: {e}"),
    err_image_eval: |e| format!("Análisis de la imagen: {e}"),
    answer_truncated: |n| format!("La respuesta de la IA se cortó tras {n} tokens"),
    err_chat_template: |e| format!("Plantilla de chat: {e}"),
    err_context: |n, e| format!("Contexto de la IA ({n} tokens): {e}"),
    err_eval: |e| format!("Cálculo de la IA: {e}"),
    cancelled: "Cancelado",
    gen_stats: |input, input_s, output, output_s| {
        format!("Entrada {input} tokens en {input_s:.1} s, respuesta {output} tokens en {output_s:.1} s")
            .replace('.', ",")
    },

    progress_of: |label, done, total| format!("{label}: {done} de {total}"),
    progress_done: |label, done| format!("{label}: {done}"),
    decimal_separator: ",",
    unit_gb: "GB",
    unit_mb: "MB",
    cannot_create: |dir| format!("No se puede crear {dir}"),
    download_http_failed: |url, code| format!("Falló la descarga de {url} (HTTP {code})"),
    download_failed: |url, e| format!("Falló la descarga de {url}: {e}"),
    cannot_write: |path| format!("No se puede escribir {path}"),
    download_cancelled: "Descarga cancelada",
    download_interrupted: |e| format!("Descarga interrumpida: {e}"),
    download_incomplete: |done, total| {
        format!("Descarga incompleta ({done} de {total}) – si vuelves a intentarlo, se reanuda")
    },
    cannot_rename: |path| format!("No se puede renombrar {path}"),

    server_unreachable_at: |url, e| format!("No se puede conectar con el servidor de IA ({url}): {e}"),
    server_no_model: "No hay ningún modelo cargado en el servidor de IA",
    server_status: |code, text| format!("El servidor de IA responde con {code}: {text}"),
    server_unreachable: |e| format!("No se puede conectar con el servidor de IA: {e}"),
    server_unexpected: "Respuesta inesperada del servidor de IA",
    server_rejected: |text| format!("El servidor de IA rechaza la solicitud: {text}"),
    answer_no_json: "La respuesta de la IA no contiene JSON",
    answer_incomplete_json: "La respuesta de la IA no contiene un JSON completo",
    answer_invalid_json: "La respuesta de la IA no es un JSON válido",
    answer_bad_format: "La respuesta de la IA tiene un formato inesperado",
};

pub static IT: Texts = Texts {
    cpu_no_gpu: "Processore (nessuna GPU)",
    embedded_on: "IA integrata su ",
    model_ready: |name| format!("{name} è scaricato e pronto all'uso"),
    download_of: |name, e| format!("Download di {name}: {e}"),

    runs_locally: "L'IA funziona su questo computer, nulla ne esce. Basta il normale driver grafico.",
    graphics_card: "Scheda grafica",
    check_again: "Controlla di nuovo",
    gpu_checking: "Verifica già in corso",
    searching_gpus: "Ricerca delle schede grafiche …",
    device_line: |kind, summary| format!("{kind}: {summary}"),
    device_free: |mem| format!(", {mem} liberi"),
    in_use: "in uso",
    no_gpu_hint: "Nessuna scheda grafica utilizzabile. Installa un driver grafico aggiornato (AMD Adrenalin, o Mesa \
                  RADV su Linux) e controlla di nuovo – altrimenti l'IA lavora sul processore (più lento).",
    recommendation: |reason| format!("Consiglio: {reason}"),

    model: "Modello",
    own_model_file: "È impostato un file di modello proprio (in «Avanzate») e ha la precedenza.",
    auto_for_hardware: |name| format!("Automatico (per questo hardware: {name})"),
    auto_matching: "Automatico, in base all'hardware",
    recommended: "consigliato",
    cancel: "Annulla",
    installed: "installato",
    delete: "Elimina",
    delete_hover: "Rimuovi i file del modello",
    download: "Scarica",
    download_running: "È già in corso un altro download",
    source_and_location: |dir| {
        format!(
            "Fonte: Hugging Face (Qwen). I download interrotti riprendono da dove si erano fermati. Percorso: {dir}"
        )
    },
    delete_title: |name| format!("Eliminare {name}?"),
    delete_text: |size| {
        format!(
            "I file del modello ({size}) verranno rimossi. Per usare di nuovo il modello dovrai scaricarlo \
             un'altra volta."
        )
    },

    advanced: "Avanzate",
    llama_cpp_hint: "L'IA funziona con llama.cpp. Le schede grafiche AMD, NVIDIA e Intel vengono usate tramite Vulkan \
                     – ROCm o CUDA non servono.",
    gpu_layers: "Livelli sulla GPU",
    layers_all: "tutti sulla GPU",
    layers_none: "solo processore",
    layers_some: |n| {
        if n == 1 {
            "1 livello sulla GPU, il resto sul processore".to_string()
        } else {
            format!("{n} livelli sulla GPU, il resto sul processore")
        }
    },
    context_length: "Lunghezza del contesto",
    context_hint: "Token per immagine, testo e risposta",
    max_chars: "Max. caratteri di testo",
    own_gguf: "File GGUF proprio",
    own_gguf_hint: "vuoto = modello dall'elenco",
    choose_model: "Scegli il modello",
    image_projector: "Proiettore di immagini",
    projector_hint: "mmproj-….gguf (per le immagini)",
    choose_projector: "Scegli il proiettore di immagini",

    note_fast: "veloce, utilizzabile anche senza GPU",
    note_balanced: "bilanciato, da circa 6 GB di memoria grafica",
    note_best: "qualità migliore, da circa 10 GB di memoria grafica",

    llama_start_failed: |e| format!("Impossibile avviare llama.cpp: {e}"),
    integrated_graphics: "Grafica integrata",
    processor: "Processore",
    memory_shared: "di memoria condivisa",
    memory_vram: "di VRAM",
    reason_no_gpu: "Nessuna GPU utilizzabile trovata – il modello piccolo funziona anche sul processore.",
    reason_gpu: |gpu, mem, label, model, fits| {
        let fit = if fits { "ci sta per intero" } else { "gira in parte sul processore" };
        format!("{gpu} con {mem} {label} → {model} {fit}.")
    },

    model_file_missing: |path| format!("File del modello {path} non trovato"),
    projector_missing: |path| format!("Proiettore di immagini {path} non trovato"),
    unknown_model: |id| format!("Modello sconosciuto «{id}»"),
    not_downloaded: |name| {
        format!("{name} non è ancora scaricato – scaricalo in Impostazioni › IA › Modello e scheda grafica.")
    },
    no_model_yet: |name, size| {
        format!(
            "Nessun modello di IA ancora scaricato. Consigliato per questo computer: {name} ({size}) – scaricalo in \
             Impostazioni › IA › Modello e scheda grafica."
        )
    },
    download_corrupt: |file| format!("{file} è arrivato danneggiato – scaricalo di nuovo"),
    cannot_delete: |path| format!("Impossibile eliminare {path}"),

    prompt_too_long: |tokens, budget| {
        format!(
            "Input troppo lungo ({tokens} token, contesto {budget}) – aumenta la lunghezza del contesto o invia meno \
             testo"
        )
    },
    model_load_failed: |path, e| format!("Impossibile caricare il modello {path}: {e}"),
    gpu_load_failed: |e| format!("L'IA gira sul processore, caricamento sulla GPU non riuscito: {e}"),
    projector_path_invalid: "Il percorso del proiettore di immagini non è un testo valido",
    projector_not_loaded: |e| format!("Proiettore di immagini non caricato, l'IA legge solo il testo: {e}"),
    device_via: |name, backend| format!("{name} tramite {backend}"),
    err_json_grammar: |e| format!("Grammatica JSON: {e}"),
    err_image: |e| format!("Immagine: {e}"),
    err_image_tokens: |e| format!("Tokenizzazione dell'immagine: {e}"),
    err_image_eval: |e| format!("Analisi dell'immagine: {e}"),
    answer_truncated: |n| format!("La risposta dell'IA è stata troncata dopo {n} token"),
    err_chat_template: |e| format!("Modello di chat: {e}"),
    err_context: |n, e| format!("Contesto dell'IA ({n} token): {e}"),
    err_eval: |e| format!("Calcolo dell'IA: {e}"),
    cancelled: "Annullato",
    gen_stats: |input, input_s, output, output_s| {
        format!("Input {input} token in {input_s:.1} s, risposta {output} token in {output_s:.1} s").replace('.', ",")
    },

    progress_of: |label, done, total| format!("{label}: {done} di {total}"),
    progress_done: |label, done| format!("{label}: {done}"),
    decimal_separator: ",",
    unit_gb: "GB",
    unit_mb: "MB",
    cannot_create: |dir| format!("Impossibile creare {dir}"),
    download_http_failed: |url, code| format!("Download da {url} non riuscito (HTTP {code})"),
    download_failed: |url, e| format!("Download da {url} non riuscito: {e}"),
    cannot_write: |path| format!("Impossibile scrivere {path}"),
    download_cancelled: "Download annullato",
    download_interrupted: |e| format!("Download interrotto: {e}"),
    download_incomplete: |done, total| format!("Download incompleto ({done} di {total}) – riprova per riprenderlo"),
    cannot_rename: |path| format!("Impossibile rinominare {path}"),

    server_unreachable_at: |url, e| format!("Server IA non raggiungibile ({url}): {e}"),
    server_no_model: "Nessun modello caricato nel server IA",
    server_status: |code, text| format!("Il server IA risponde con {code}: {text}"),
    server_unreachable: |e| format!("Server IA non raggiungibile: {e}"),
    server_unexpected: "Risposta inattesa dal server IA",
    server_rejected: |text| format!("Il server IA rifiuta la richiesta: {text}"),
    answer_no_json: "La risposta dell'IA non contiene JSON",
    answer_incomplete_json: "La risposta dell'IA non contiene un JSON completo",
    answer_invalid_json: "La risposta dell'IA non è un JSON valido",
    answer_bad_format: "La risposta dell'IA ha un formato inatteso",
};
