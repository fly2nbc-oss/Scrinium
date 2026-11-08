//! Client for OpenAI-compatible local servers (LM Studio, Ollama, llama.cpp).

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::config::LlmConfig;
use crate::i18n::ai;

pub struct LlmClient {
    base_url: String,
    api_key: String,
    agent: ureq::Agent,
}

/// Structured answer of the AI (same order as in the schema: understand first, then classify).
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct LlmAnalysis {
    pub summary: String,
    pub doctype: String,
    /// The AI's own document type when it chose [`DOCTYPE_OTHER`].
    pub doctype_new: String,
    pub sender: String,
    pub date: String,
    #[serde(alias = "title")]
    pub subject: String,
    pub person: String,
    pub extra: String,
    pub target_folder: String,
    pub confidence: f32,
}

/// Choice lists for the response schema. Empty lists mean free text.
#[derive(Debug, Clone, Default)]
pub struct SchemaSpec<'a> {
    pub doctypes: &'a [String],
    pub extras: &'a [String],
    pub persons: Vec<String>,
    /// Fixed folder list (only when new folders are not allowed).
    pub folders: Option<&'a [String]>,
    /// Without target folder: when the folder is chosen in stages, the AI only describes the document.
    pub without_folder: bool,
    /// Without summary: it is already known from the first step.
    pub without_summary: bool,
}

pub struct LlmRequest<'a> {
    pub model: &'a str,
    pub system: &'a str,
    pub user: &'a str,
    /// JPEG as Base64 for vision models.
    pub image_jpeg_b64: Option<&'a str>,
    pub schema: &'a Value,
}

impl LlmClient {
    pub fn new(cfg: &LlmConfig) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(3))
            .timeout(Duration::from_secs(cfg.timeout_secs.max(5)))
            .build();
        Self { base_url: cfg.base_url.trim_end_matches('/').to_string(), api_key: cfg.api_key.clone(), agent }
    }

    fn request(&self, method: &str, path: &str) -> ureq::Request {
        let req = self.agent.request(method, &format!("{}{}", self.base_url, path));
        if self.api_key.is_empty() { req } else { req.set("Authorization", &format!("Bearer {}", self.api_key)) }
    }

    /// Loaded or available models (`GET /models`).
    pub fn list_models(&self) -> Result<Vec<String>> {
        let resp: Value = self
            .request("GET", "/models")
            .timeout(Duration::from_secs(5))
            .call()
            .map_err(|e| anyhow!((ai::t().server_unreachable_at)(&self.base_url, &short_error(e))))?
            .into_json()?;
        let models = resp["data"]
            .as_array()
            .map(|a| a.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect())
            .unwrap_or_default();
        Ok(models)
    }

    /// The configured model, or the server's first chat model.
    pub fn resolve_model(&self, configured: &str) -> Result<String> {
        if !configured.trim().is_empty() {
            return Ok(configured.trim().to_string());
        }
        self.list_models()?
            .into_iter()
            .find(|m| !m.to_lowercase().contains("embed"))
            .ok_or_else(|| anyhow!(ai::t().server_no_model))
    }

    pub fn analyze(&self, req: &LlmRequest) -> Result<LlmAnalysis> {
        let user_content = match req.image_jpeg_b64 {
            Some(b64) => json!([
                { "type": "text", "text": req.user },
                { "type": "image_url", "image_url": { "url": format!("data:image/jpeg;base64,{b64}") } }
            ]),
            None => json!(req.user),
        };
        let mut body = json!({
            "model": req.model,
            "temperature": 0.1,
            "max_tokens": 800,
            "stream": false,
            "messages": [
                { "role": "system", "content": req.system },
                { "role": "user", "content": user_content }
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": { "name": "dokument_analyse", "strict": true, "schema": req.schema }
            }
        });

        let content = match self.chat(&body) {
            Ok(c) => c,
            // Some servers/models do not support json_schema; then retry without an enforced format.
            Err(ChatError::Rejected(_)) => {
                if let Some(body) = body.as_object_mut() {
                    body.remove("response_format");
                }
                self.chat(&body).map_err(ChatError::into_anyhow)?
            }
            Err(e) => return Err(e.into_anyhow()),
        };
        parse_analysis(&content)
    }

    /// Request with a JSON schema, optionally with a page image; returns the JSON from the answer.
    pub fn complete_json(
        &self,
        model: &str,
        system: &str,
        user: &str,
        image_jpeg_b64: Option<&str>,
        schema: &Value,
        max_tokens: usize,
    ) -> Result<Value> {
        let user_content = match image_jpeg_b64 {
            Some(b64) => json!([
                { "type": "text", "text": user },
                { "type": "image_url", "image_url": { "url": format!("data:image/jpeg;base64,{b64}") } }
            ]),
            None => json!(user),
        };
        let mut body = json!({
            "model": model,
            "temperature": 0.1,
            "max_tokens": max_tokens,
            "stream": false,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user_content }
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": { "name": "antwort", "strict": true, "schema": schema }
            }
        });
        let content = match self.chat(&body) {
            Ok(c) => c,
            Err(ChatError::Rejected(_)) => {
                if let Some(body) = body.as_object_mut() {
                    body.remove("response_format");
                }
                self.chat(&body).map_err(ChatError::into_anyhow)?
            }
            Err(e) => return Err(e.into_anyhow()),
        };
        extract_json(&content)
    }

    fn chat(&self, body: &Value) -> std::result::Result<String, ChatError> {
        let t = ai::t();
        let resp = self.request("POST", "/chat/completions").send_json(body.clone());
        let resp: Value = match resp {
            Ok(r) => r.into_json().map_err(|e| ChatError::Other(e.into()))?,
            Err(ureq::Error::Status(code, r)) if code == 400 || code == 422 => {
                return Err(ChatError::Rejected(r.into_string().unwrap_or_default()));
            }
            Err(ureq::Error::Status(code, r)) => {
                let text = r.into_string().unwrap_or_default();
                return Err(ChatError::Other(anyhow!((t.server_status)(code, &truncate(&text, 300)))));
            }
            Err(e) => return Err(ChatError::Other(anyhow!((t.server_unreachable)(&short_error(e))))),
        };
        resp["choices"][0]["message"]["content"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| ChatError::Other(anyhow!(t.server_unexpected)))
    }
}

enum ChatError {
    Rejected(String),
    Other(anyhow::Error),
}

impl ChatError {
    fn into_anyhow(self) -> anyhow::Error {
        match self {
            ChatError::Rejected(text) => anyhow!((ai::t().server_rejected)(&truncate(&text, 300))),
            ChatError::Other(e) => e,
        }
    }
}

fn short_error(e: ureq::Error) -> String {
    match e {
        ureq::Error::Transport(t) => t.kind().to_string(),
        other => other.to_string(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() } else { format!("{} …", s.chars().take(max).collect::<String>()) }
}

/// The "andere" (other) choice in the list of document types: the AI's own term is then in `doctype_new`.
pub const DOCTYPE_OTHER: &str = "andere";

/// Largest folder list that goes into the schema as a fixed choice (keeps the grammar manageable).
const MAX_FOLDER_ENUM: usize = 800;

/// Response schema. The field order is deliberate: the AI first summarises, then determines
/// type, sender, date and subject, and the folder last.
pub fn response_schema(spec: &SchemaSpec) -> Value {
    let choice = |values: &[String], allow_empty: bool, description: &str| {
        let mut v: Vec<&str> = values.iter().map(|s| s.as_str()).filter(|s| !s.trim().is_empty()).collect();
        if v.is_empty() {
            return json!({ "type": "string", "description": description });
        }
        if allow_empty {
            v.insert(0, "");
        }
        v.dedup();
        json!({ "type": "string", "enum": v, "description": description })
    };
    let person_hint = if spec.persons.is_empty() {
        "Betroffene Person oder leer".to_string()
    } else {
        format!("Person, um die es selbst geht: {}; sonst leer", spec.persons.join(", "))
    };
    let folder = match spec.folders {
        Some(folders) if !folders.is_empty() && folders.len() <= MAX_FOLDER_ENUM => {
            choice(folders, true, "Zielordner aus der Liste")
        }
        _ => json!({ "type": "string", "description": "Zielordner aus der Liste" }),
    };
    // Fixed list plus "andere" (other): if no term really fits, the AI gives its own in `doctype_new`.
    let mut doctypes = spec.doctypes.to_vec();
    if !doctypes.is_empty() {
        doctypes.push(DOCTYPE_OTHER.to_string());
    }
    let mut schema = json!({
        "type": "object",
        "properties": {
            "summary": { "type": "string", "description": "Ein bis zwei Sätze: worum es geht" },
            "doctype": choice(&doctypes, false, "Dokumentart"),
            "doctype_new": { "type": "string", "description": "Nur bei doctype „andere“: eigene Dokumentart, sonst leer" },
            "sender": { "type": "string", "description": "Absender als Kurzname" },
            "date": { "type": "string", "description": "Dokumentdatum als YYYY-MM-DD, YYYY-MM, YYYY oder leer" },
            "subject": { "type": "string", "description": "Betreff, ein bis vier Wörter, Zeitraum zuerst" },
            "person": { "type": "string", "description": person_hint },
            "extra": choice(spec.extras, true, "Zusatz oder leer"),
            "target_folder": folder,
            "confidence": { "type": "number" }
        },
        "required": ["summary", "doctype", "doctype_new", "sender", "date", "subject", "person", "extra", "target_folder", "confidence"],
        "additionalProperties": false
    });
    let fixed_doctypes = !spec.doctypes.is_empty();
    for (skip, key) in
        [(spec.without_folder, "target_folder"), (spec.without_summary, "summary"), (!fixed_doctypes, "doctype_new")]
    {
        if skip {
            if let Some(properties) = schema["properties"].as_object_mut() {
                properties.remove(key);
            }
            if let Some(required) = schema["required"].as_array_mut() {
                required.retain(|k| k != key);
            }
        }
    }
    schema
}

/// Answer of a choice when nothing in the list fits.
pub const NO_CHOICE: &str = "keiner";

/// Schema of a choice in one stage: a short reason, then exactly one list entry or [`NO_CHOICE`].
pub fn choice_schema(options: &[String]) -> Value {
    let mut v: Vec<&str> = options.iter().map(String::as_str).collect();
    v.push(NO_CHOICE);
    json!({
        "type": "object",
        "properties": {
            "reason": { "type": "string", "description": "Ein kurzer Satz, warum" },
            "choice": { "type": "string", "enum": v }
        },
        "required": ["reason", "choice"],
        "additionalProperties": false
    })
}

/// Extracts the JSON object from the answer, even when the model writes code fences
/// or thinking sections (`<think>…</think>`) before it.
pub fn extract_json(content: &str) -> Result<Value> {
    let mut text = content;
    if let Some(end) = text.rfind("</think>") {
        text = &text[end + "</think>".len()..];
    }
    let t = ai::t();
    let start = text.find('{').context(t.answer_no_json)?;
    let end = text.rfind('}').context(t.answer_incomplete_json)?;
    if end <= start {
        bail!(t.answer_incomplete_json);
    }
    serde_json::from_str(&text[start..=end]).context(t.answer_invalid_json)
}

/// AI answer of a document analysis.
pub fn parse_analysis(content: &str) -> Result<LlmAnalysis> {
    analysis_from_value(extract_json(content)?)
}

/// AI answer of a document analysis from already parsed JSON.
pub fn analysis_from_value(value: Value) -> Result<LlmAnalysis> {
    let mut a: LlmAnalysis = serde_json::from_value(value).context(ai::t().answer_bad_format)?;
    for f in [
        &mut a.date,
        &mut a.sender,
        &mut a.doctype,
        &mut a.subject,
        &mut a.person,
        &mut a.extra,
        &mut a.summary,
        &mut a.target_folder,
    ] {
        *f = f.trim().to_string();
    }
    a.confidence = a.confidence.clamp(0.0, 1.0);
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    /// Minimal HTTP server that answers like LM Studio and records the requests.
    fn mock_server(responses: Vec<(u16, String)>) -> (String, mpsc::Receiver<(String, String)>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();
                let mut len = 0usize;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                }
                let mut req_body = vec![0; len];
                reader.read_exact(&mut req_body).unwrap();
                tx.send((request_line, String::from_utf8(req_body).unwrap())).unwrap();
                let resp = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(resp.as_bytes()).unwrap();
            }
        });
        (format!("http://{addr}/v1"), rx)
    }

    fn chat_response(content: &str) -> String {
        json!({ "choices": [{ "message": { "role": "assistant", "content": content } }] }).to_string()
    }

    fn client(base_url: String) -> LlmClient {
        LlmClient::new(&LlmConfig { base_url, ..Default::default() })
    }

    #[test]
    fn analyze_with_vision_and_schema() {
        let answer = r#"{"summary":"Abrechnung","doctype":"Rechnung","sender":"Stadtwerke Musterstadt","date":"2024-03-12","subject":"2023","person":"","extra":"","target_folder":"Wohnung/Nebenkosten","confidence":0.9}"#;
        let (url, rx) = mock_server(vec![(200, chat_response(answer))]);
        let schema = response_schema(&SchemaSpec::default());
        let a = client(url)
            .analyze(&LlmRequest {
                model: "qwen",
                system: "sys",
                user: "text",
                image_jpeg_b64: Some("QUJD"),
                schema: &schema,
            })
            .unwrap();
        assert_eq!(a.sender, "Stadtwerke Musterstadt");
        assert_eq!(a.target_folder, "Wohnung/Nebenkosten");
        let (line, body) = rx.recv().unwrap();
        assert!(line.starts_with("POST /v1/chat/completions"));
        let body: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["response_format"]["type"], "json_schema");
        assert_eq!(body["messages"][1]["content"][1]["image_url"]["url"], "data:image/jpeg;base64,QUJD");
    }

    #[test]
    fn retries_without_schema_when_rejected() {
        let answer = "```json\n{\"date\":\"2023-01-05\",\"sender\":\"AOK\",\"doctype\":\"Bescheid\",\"title\":\"Beitrag\",\"summary\":\"\",\"target_folder\":\"\",\"confidence\":2}\n```";
        let (url, rx) =
            mock_server(vec![(400, "{\"error\":\"json_schema unsupported\"}".into()), (200, chat_response(answer))]);
        let schema = response_schema(&SchemaSpec::default());
        let a = client(url)
            .analyze(&LlmRequest { model: "m", system: "s", user: "u", image_jpeg_b64: None, schema: &schema })
            .unwrap();
        assert_eq!(a.sender, "AOK");
        assert_eq!(a.subject, "Beitrag", "the older field title is accepted");
        assert_eq!(a.confidence, 1.0);
        let (_, first) = rx.recv().unwrap();
        let (_, second) = rx.recv().unwrap();
        assert!(first.contains("response_format"));
        assert!(!second.contains("response_format"));
        assert!(second.contains("\"content\":\"u\""));
    }

    #[test]
    fn lists_and_resolves_models() {
        let models = json!({ "data": [{ "id": "text-embedding-nomic" }, { "id": "qwen2.5-vl-7b" }] }).to_string();
        let (url, _rx) = mock_server(vec![(200, models)]);
        assert_eq!(client(url).resolve_model("").unwrap(), "qwen2.5-vl-7b");
    }

    #[test]
    fn unreachable_server_is_an_error() {
        let c = client("http://127.0.0.1:9/v1".into());
        assert!(c.list_models().is_err());
    }

    #[test]
    fn schema_has_choices_in_answer_order() {
        let doctypes = vec!["Rechnung".to_string(), "Urkunde".to_string()];
        let extras = vec!["unterschrieben".to_string()];
        let folders = vec!["01_Versicherung".to_string(), "02_Haus/_Wasser_Abfall".to_string()];
        let spec = SchemaSpec {
            doctypes: &doctypes,
            extras: &extras,
            persons: vec!["Max".into()],
            folders: Some(&folders),
            without_folder: false,
            without_summary: false,
        };
        let schema = response_schema(&spec);
        let keys: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
        assert_eq!(keys[0], "summary");
        assert_eq!(keys[8], "target_folder");
        assert_eq!(schema["properties"]["doctype"]["enum"], json!(["Rechnung", "Urkunde", "andere"]));
        assert_eq!(keys[2], "doctype_new", "own term right after the choice");
        assert_eq!(schema["properties"]["extra"]["enum"], json!(["", "unterschrieben"]));
        assert_eq!(schema["properties"]["target_folder"]["enum"][2], "02_Haus/_Wasser_Abfall");
        assert!(schema["properties"]["person"]["description"].as_str().unwrap().contains("Max"));
        let free = response_schema(&SchemaSpec::default());
        assert!(free["properties"]["doctype"].get("enum").is_none());
        assert!(free["properties"].get("doctype_new").is_none(), "without a list the type is free anyway");
        let steps = response_schema(&SchemaSpec { without_folder: true, ..SchemaSpec::default() });
        assert!(steps["properties"].get("target_folder").is_none());
        assert!(!steps["required"].as_array().unwrap().iter().any(|k| k == "target_folder"));
        let fields =
            response_schema(&SchemaSpec { without_folder: true, without_summary: true, ..SchemaSpec::default() });
        assert!(fields["properties"].get("summary").is_none() && fields["properties"].get("doctype").is_some());
        assert!(!fields["required"].as_array().unwrap().iter().any(|k| k == "summary"));
        let pick = choice_schema(&["A".into(), "B".into()]);
        assert_eq!(pick["properties"]["choice"]["enum"], json!(["A", "B", "keiner"]));
    }

    #[test]
    fn parses_reasoning_output() {
        let a = parse_analysis("<think>{ hmm }</think>\n{\"sender\":\" ACME \",\"date\":\"2020-02-02\"}").unwrap();
        assert_eq!(a.sender, "ACME");
        assert!(parse_analysis("keine Ahnung").is_err());
    }
}
