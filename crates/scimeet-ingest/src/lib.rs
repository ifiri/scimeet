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

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Client;
    use scimeet_core::{Document, DocumentId, ScimeetConfig, SourceKind};

    #[test]
    fn document_fingerprint_stable() {
        let doc = Document {
            id: DocumentId("pmid:1".to_string()),
            source: SourceKind::PubMed,
            title: "T".to_string(),
            abstract_text: "A".to_string(),
            doi: None,
            pmid: Some("1".to_string()),
            url: None,
            published: None,
        };
        let a = document_fingerprint(&doc);
        let b = document_fingerprint(&doc);
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn document_fingerprint_changes_with_content() {
        let d1 = Document {
            id: DocumentId("x".to_string()),
            source: SourceKind::PubMed,
            title: "same".to_string(),
            abstract_text: "one".to_string(),
            doi: None,
            pmid: None,
            url: None,
            published: None,
        };
        let d2 = Document {
            id: DocumentId("x".to_string()),
            source: SourceKind::PubMed,
            title: "same".to_string(),
            abstract_text: "two".to_string(),
            doi: None,
            pmid: None,
            url: None,
            published: None,
        };
        assert_ne!(document_fingerprint(&d1), document_fingerprint(&d2));
    }

    #[tokio::test]
    async fn maybe_translate_passes_through_when_ingest_off() {
        let mut c = ScimeetConfig::defaults();
        c.translate_on_ingest = false;
        let translator = OllamaTranslator::new(c.clone(), Client::new());
        let out = maybe_translate_chunk(&c, &translator, "no network").await.unwrap();
        assert_eq!(out, "no network");
    }
}
