use reqwest::Client;
use scimeet_core::{ScimeetConfig, ScimeetError};
use scimeet_index::{IndexedChunk, OllamaEmbeddings, VectorStore};
use scimeet_translate::OllamaTranslator;
use serde::{Deserialize, Serialize};
use tracing::instrument;

pub struct RagEngine {
    config: ScimeetConfig,
    embeddings: OllamaEmbeddings,
    translator: OllamaTranslator,
    client: Client,
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

impl RagEngine {
    pub fn new(config: ScimeetConfig, client: Client) -> Self {
        let embeddings = OllamaEmbeddings::new(config.clone(), client.clone());
        let translator = OllamaTranslator::new(config.clone(), client.clone());
        Self {
            config,
            embeddings,
            translator,
            client,
        }
    }

    #[instrument(skip(self))]
    pub async fn embed_query(&self, question: &str) -> Result<Vec<f32>, ScimeetError> {
        let text = if scimeet_translate::should_translate_query(&self.config, question) {
            match self.translator.to_english(question).await {
                Ok(t) if !t.is_empty() => t,
                Ok(_) if self.config.translate_fallback_to_original => question.to_string(),
                Ok(_) => return Err(ScimeetError::Ollama("empty translation".to_string())),
                Err(_) if self.config.translate_fallback_to_original => question.to_string(),
                Err(e) => return Err(e),
            }
        } else {
            question.to_string()
        };
        self.embeddings.embed(&text).await
    }

    #[instrument(skip(self, store))]
    pub async fn retrieve(
        &self,
        store: &VectorStore,
        question: &str,
        top_k: usize,
    ) -> Result<Vec<IndexedChunk>, ScimeetError> {
        let qvec = self.embed_query(question).await?;
        store.search(&qvec, top_k).await
    }

    pub async fn answer(
        &self,
        store: &VectorStore,
        question: &str,
        top_k: usize,
    ) -> Result<String, ScimeetError> {
        let hits = self.retrieve(store, question, top_k).await?;
        self.answer_from_hits(question, &hits).await
    }

    #[instrument(skip(self, hits))]
    pub async fn answer_from_hits(
        &self,
        question: &str,
        hits: &[IndexedChunk],
    ) -> Result<String, ScimeetError> {
        let context = build_context(hits);
        let lang_note = if scimeet_translate::should_translate_query(&self.config, question) {
            "Answer in the same language as the user's question."
        } else {
            "Answer in clear English."
        };
        let system = format!(
            "You are a research assistant. Answer only using the CONTEXT below. Cite sources by title or PMID/DOI when present. If context is insufficient, say so. {}",
            lang_note
        );
        let user = format!("QUESTION:\n{}\n\nCONTEXT:\n{}", question, context);
        let url = format!(
            "{}/api/chat",
            self.config.ollama_base.trim_end_matches('/')
        );
        let body = ChatRequest {
            model: self.config.chat_model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user,
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
            return Err(ScimeetError::Ollama(format!("chat http: {}", t)));
        }
        let parsed: ChatResponse = res
            .json()
            .await
            .map_err(|e| ScimeetError::Ollama(e.to_string()))?;
        Ok(parsed.message.content.trim().to_string())
    }
}

fn build_context(hits: &[IndexedChunk]) -> String {
    let mut s = String::new();
    for (i, h) in hits.iter().enumerate() {
        let pmid = h.meta.pmid.as_deref().unwrap_or("—");
        let doi = h.meta.doi.as_deref().unwrap_or("—");
        let url = h.meta.url.as_deref().unwrap_or("—");
        s.push_str(&format!(
            "[{}] score={:.3}\nTitle: {}\nPMID: {} DOI: {}\nURL: {}\n{}\n\n",
            i + 1,
            h.score,
            h.meta.title,
            pmid,
            doi,
            url,
            h.text
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use scimeet_core::{ChunkMeta, DocumentId, SourceKind};

    fn sample_hit(text: &str, score: f32) -> IndexedChunk {
        IndexedChunk {
            id: "id1".to_string(),
            text: text.to_string(),
            meta: ChunkMeta {
                document_id: DocumentId("d:1".to_string()),
                title: "T".to_string(),
                source: SourceKind::PubMed,
                doi: Some("10.1/x".to_string()),
                pmid: Some("1".to_string()),
                url: Some("https://x".to_string()),
                chunk_index: 0,
            },
            score,
        }
    }

    #[test]
    fn build_context_includes_scores_and_text() {
        let hits = vec![
            sample_hit("chunk body one", 0.9),
            sample_hit("chunk body two", 0.5),
        ];
        let ctx = build_context(&hits);
        assert!(ctx.contains("score=0.900"));
        assert!(ctx.contains("chunk body one"));
        assert!(ctx.contains("PMID: 1"));
        assert!(ctx.contains("DOI: 10.1/x"));
        assert!(ctx.contains("URL: https://x"));
    }

    #[test]
    fn build_context_empty() {
        assert_eq!(build_context(&[]), "");
    }
}
