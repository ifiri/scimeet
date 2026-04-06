use crate::default_params as dp;
use crate::ScimeetError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
    pub connect_timeout_secs: u64,
    pub embed_dim: usize,
    pub http_pool_max_idle_per_host: usize,
    pub http_pool_idle_timeout_secs: u64,
    pub http_user_agent: String,
}

impl ScimeetConfig {
    pub fn defaults() -> Self {
        Self {
            ollama_base: dp::OLLAMA_BASE.to_string(),
            embed_model: dp::EMBED_MODEL.to_string(),
            chat_model: dp::CHAT_MODEL.to_string(),
            translate_model: dp::TRANSLATE_MODEL.to_string(),
            translate_on_query: dp::TRANSLATE_ON_QUERY,
            translate_on_ingest: dp::TRANSLATE_ON_INGEST,
            translate_fallback_to_original: dp::TRANSLATE_FALLBACK_TO_ORIGINAL,
            data_dir: PathBuf::from(dp::DATA_DIR),
            ncbi_api_key: None,
            request_timeout_secs: dp::REQUEST_TIMEOUT_SECS,
            connect_timeout_secs: dp::CONNECT_TIMEOUT_SECS,
            embed_dim: dp::EMBED_DIM,
            http_pool_max_idle_per_host: dp::HTTP_POOL_MAX_IDLE_PER_HOST,
            http_pool_idle_timeout_secs: dp::HTTP_POOL_IDLE_TIMEOUT_SECS,
            http_user_agent: dp::HTTP_USER_AGENT.to_string(),
        }
    }

    pub fn from_toml_str(toml: &str) -> Result<Self, ScimeetError> {
        toml::from_str(toml).map_err(|e| ScimeetError::Config(e.to_string()))
    }

    pub fn lancedb_path(&self) -> PathBuf {
        self.data_dir.join("lancedb")
    }

    pub fn index_path(&self) -> PathBuf {
        self.lancedb_path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_params as dp;

    #[test]
    fn defaults_match_constants() {
        let c = ScimeetConfig::defaults();
        assert_eq!(c.ollama_base, dp::OLLAMA_BASE);
        assert_eq!(c.embed_model, dp::EMBED_MODEL);
        assert_eq!(c.chat_model, dp::CHAT_MODEL);
        assert_eq!(c.translate_model, dp::TRANSLATE_MODEL);
        assert_eq!(c.translate_on_query, dp::TRANSLATE_ON_QUERY);
        assert_eq!(c.data_dir, PathBuf::from(dp::DATA_DIR));
        assert_eq!(c.embed_dim, dp::EMBED_DIM);
        assert_eq!(c.http_user_agent, dp::HTTP_USER_AGENT);
    }

    #[test]
    fn toml_roundtrip_defaults() {
        let d = ScimeetConfig::defaults();
        let s = toml::to_string(&d).unwrap();
        let c = ScimeetConfig::from_toml_str(&s).unwrap();
        assert_eq!(d, c);
    }
}
