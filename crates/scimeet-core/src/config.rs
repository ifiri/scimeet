use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScimeetConfig {
    pub ollama_base: String,
    pub embed_model: String,
    pub chat_model: String,
    pub translate_model: String,
    pub translate_on_query: bool,
    pub translate_on_ingest: bool,
    pub translate_fallback_to_original: bool,
    pub data_dir: PathBuf,
    pub ncbi_api_key: Option<String>,
    pub request_timeout_secs: u64,
}

impl Default for ScimeetConfig {
    fn default() -> Self {
        Self {
            ollama_base: "http://127.0.0.1:11434".to_string(),
            embed_model: "nomic-embed-text".to_string(),
            chat_model: "llama3.1:8b".to_string(),
            translate_model: "llama3.1:8b".to_string(),
            translate_on_query: true,
            translate_on_ingest: false,
            translate_fallback_to_original: true,
            data_dir: PathBuf::from("data"),
            ncbi_api_key: std::env::var("NCBI_API_KEY").ok(),
            request_timeout_secs: 120,
        }
    }
}

impl ScimeetConfig {
    pub fn index_path(&self) -> PathBuf {
        self.data_dir.join("index.sqlite")
    }

    pub fn from_env_overrides(mut self) -> Self {
        if let Ok(v) = std::env::var("OLLAMA_HOST") {
            self.ollama_base = v;
        }
        if let Ok(v) = std::env::var("SCIMEET_DATA_DIR") {
            self.data_dir = PathBuf::from(v);
        }
        self
    }
}
