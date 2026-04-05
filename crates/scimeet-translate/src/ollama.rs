use reqwest::Client;
use scimeet_core::{ScimeetConfig, ScimeetError};
use serde::{Deserialize, Serialize};

pub struct OllamaTranslator {
    client: Client,
    config: ScimeetConfig,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: String,
}

impl OllamaTranslator {
    pub fn new(config: ScimeetConfig) -> Result<Self, ScimeetError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.request_timeout_secs))
            .build()
            .map_err(|e| ScimeetError::Http(e.to_string()))?;
        Ok(Self { client, config })
    }

    pub async fn to_english(&self, text: &str) -> Result<String, ScimeetError> {
        let url = format!("{}/api/chat", self.config.ollama_base.trim_end_matches('/'));
        let body = ChatRequest {
            model: self.config.translate_model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: "Translate the user text to English. Output only the translation, no quotes or explanation."
                        .to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: text.to_string(),
                },
            ],
            stream: false,
        };
        let res = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ScimeetError::Http(e.to_string()))?;
        if !res.status().is_success() {
            let t = res.text().await.unwrap_or_default();
            return Err(ScimeetError::Ollama(format!("translate http: {}", t)));
        }
        let parsed: ChatResponse = res
            .json()
            .await
            .map_err(|e| ScimeetError::Ollama(e.to_string()))?;
        Ok(parsed.message.content.trim().to_string())
    }
}
