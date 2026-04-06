use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DocumentId(pub String);

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    PubMed,
    Arxiv,
    BioRxiv,
    MedRxiv,
    CochraneViaPubMed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: DocumentId,
    pub source: SourceKind,
    pub title: String,
    pub abstract_text: String,
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub url: Option<String>,
    pub published: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkMeta {
    pub document_id: DocumentId,
    pub title: String,
    pub source: SourceKind,
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub url: Option<String>,
    pub chunk_index: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_kind_serde_roundtrip() {
        let k = SourceKind::PubMed;
        let j = serde_json::to_string(&k).unwrap();
        let back: SourceKind = serde_json::from_str(&j).unwrap();
        assert_eq!(k, back);
    }

    #[test]
    fn chunk_meta_json_roundtrip() {
        let m = ChunkMeta {
            document_id: DocumentId("d:1".to_string()),
            title: "t".to_string(),
            source: SourceKind::Arxiv,
            doi: None,
            pmid: Some("9".to_string()),
            url: None,
            chunk_index: 0,
        };
        let j = serde_json::to_string(&m).unwrap();
        let back: ChunkMeta = serde_json::from_str(&j).unwrap();
        assert_eq!(m.document_id, back.document_id);
        assert_eq!(m.chunk_index, back.chunk_index);
    }
}
