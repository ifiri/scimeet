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
