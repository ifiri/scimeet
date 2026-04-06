use scimeet_core::{ChunkMeta, Document};

const MAX_CHUNK_CHARS: usize = 2000;

pub fn text_for_embedding(doc: &Document) -> String {
    format!("{}\n\n{}", doc.title.trim(), doc.abstract_text.trim())
}

pub fn chunk_document(doc: &Document) -> Vec<(String, ChunkMeta)> {
    let body = doc.abstract_text.trim();
    if body.is_empty() {
        let t = doc.title.trim();
        if t.is_empty() {
            return Vec::new();
        }
        return vec![(
            format!("Title: {}", t),
            ChunkMeta {
                document_id: doc.id.clone(),
                title: doc.title.clone(),
                source: doc.source,
                doi: doc.doi.clone(),
                pmid: doc.pmid.clone(),
                url: doc.url.clone(),
                chunk_index: 0,
            },
        )];
    }
    let mut parts: Vec<String> = Vec::new();
    let paras: Vec<&str> = body.split("\n\n").map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if paras.is_empty() {
        parts.push(body.to_string());
    } else {
        let mut buf = String::new();
        for p in paras {
            if buf.len() + p.len() + 2 > MAX_CHUNK_CHARS && !buf.is_empty() {
                parts.push(buf.trim().to_string());
                buf.clear();
            }
            if !buf.is_empty() {
                buf.push_str("\n\n");
            }
            buf.push_str(p);
        }
        if !buf.is_empty() {
            parts.push(buf.trim().to_string());
        }
    }
    if parts.is_empty() {
        parts.push(body.chars().take(MAX_CHUNK_CHARS).collect());
    }
    let mut out = Vec::new();
    for (i, chunk_text) in parts.iter().enumerate() {
        let full = format!("Title: {}\n\n{}", doc.title.trim(), chunk_text);
        out.push((
            full,
            ChunkMeta {
                document_id: doc.id.clone(),
                title: doc.title.clone(),
                source: doc.source,
                doi: doc.doi.clone(),
                pmid: doc.pmid.clone(),
                url: doc.url.clone(),
                chunk_index: i,
            },
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use scimeet_core::{DocumentId, SourceKind};

    fn sample_doc(title: &str, body: &str) -> Document {
        Document {
            id: DocumentId("test:1".to_string()),
            source: SourceKind::PubMed,
            title: title.to_string(),
            abstract_text: body.to_string(),
            doi: None,
            pmid: Some("1".to_string()),
            url: None,
            published: None,
        }
    }

    #[test]
    fn chunk_empty_abstract_uses_title() {
        let doc = sample_doc("Only title", "");
        let chunks = chunk_document(&doc);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].0.contains("Only title"));
    }

    #[test]
    fn chunk_splits_paragraphs() {
        let doc = sample_doc(
            "T",
            "First paragraph.\n\nSecond paragraph.\n\nThird here.",
        );
        let chunks = chunk_document(&doc);
        assert!(chunks.len() >= 1);
        assert!(chunks[0].0.contains("Title: T"));
    }

    #[test]
    fn text_for_embedding_joins_title_and_body() {
        let doc = sample_doc("Hello", "World");
        assert_eq!(text_for_embedding(&doc), "Hello\n\nWorld");
    }
}
