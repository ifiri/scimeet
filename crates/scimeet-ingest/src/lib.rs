mod chunk;

pub use chunk::{chunk_document, text_for_embedding};

use scimeet_core::{Document, ScimeetError};
use scimeet_translate::OllamaTranslator;
use sha2::{Digest, Sha256};

pub fn document_fingerprint(doc: &Document) -> String {
    let mut h = Sha256::new();
    h.update(doc.id.0.as_bytes());
    h.update(doc.title.as_bytes());
    h.update(doc.abstract_text.as_bytes());
    format!("{:x}", h.finalize())
}

pub async fn maybe_translate_chunk(
    config: &scimeet_core::ScimeetConfig,
    translator: &OllamaTranslator,
    text: &str,
) -> Result<String, ScimeetError> {
    if scimeet_translate::should_translate_ingest(config, text) {
        match translator.to_english(text).await {
            Ok(t) if !t.is_empty() => Ok(t),
            Ok(_) if config.translate_fallback_to_original => Ok(text.to_string()),
            Ok(_) => Err(ScimeetError::Ollama("empty translation".to_string())),
            Err(_) if config.translate_fallback_to_original => Ok(text.to_string()),
            Err(e) => Err(e),
        }
    } else {
        Ok(text.to_string())
    }
}
