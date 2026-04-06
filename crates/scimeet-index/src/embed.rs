use reqwest::Client;
use scimeet_core::{ScimeetConfig, ScimeetError};
use serde::{Deserialize, Serialize};
use tracing::instrument;

pub struct OllamaEmbeddings {
    client: Client,
    config: ScimeetConfig,
}

#[derive(Serialize)]
struct EmbedRequest {
    model: String,
    prompt: String,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}

impl OllamaEmbeddings {
    pub fn new(config: ScimeetConfig, client: Client) -> Self {
        Self { client, config }
    }

    #[instrument(skip_all, fields(model = %self.config.embed_model))]
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, ScimeetError> {
        let url = format!(
            "{}/api/embeddings",
            self.config.ollama_base.trim_end_matches('/')
        );
        let body = EmbedRequest {
            model: self.config.embed_model.clone(),
            prompt: text.to_string(),
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
            return Err(ScimeetError::Ollama(format!("embed http: {}", t)));
        }
        let parsed: EmbedResponse = res
            .json()
            .await
            .map_err(|e| ScimeetError::Ollama(e.to_string()))?;
        if parsed.embedding.len() != self.config.embed_dim {
            return Err(ScimeetError::Ollama(format!(
                "embedding length {} does not match embed_dim {}",
                parsed.embedding.len(),
                self.config.embed_dim
            )));
        }
        Ok(parsed.embedding)
    }

    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ScimeetError> {
        let mut out = Vec::with_capacity(texts.len());
        for t in texts {
            out.push(self.embed(t).await?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Client;
    use scimeet_core::ScimeetConfig;

    #[tokio::test]
    async fn embed_batch_empty_no_requests() {
        let emb = OllamaEmbeddings::new(ScimeetConfig::defaults(), Client::new());
        let out = emb.embed_batch(&[]).await.unwrap();
        assert!(out.is_empty());
    }
}
