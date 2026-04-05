use rusqlite::{params, Connection, OptionalExtension};
use scimeet_core::{ChunkMeta, ScimeetError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub struct VectorStore {
    conn: Connection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexedChunk {
    pub id: String,
    pub text: String,
    pub meta: ChunkMeta,
    pub score: f32,
}

fn f32s_to_blob(v: &[f32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(v.len() * 4);
    for x in v {
        b.extend_from_slice(&x.to_le_bytes());
    }
    b
}

fn blob_to_f32s(blob: &[u8]) -> Result<Vec<f32>, ScimeetError> {
    if blob.len() % 4 != 0 {
        return Err(ScimeetError::Parse("bad embedding blob".to_string()));
    }
    let n = blob.len() / 4;
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let start = i * 4;
        let arr: [u8; 4] = blob[start..start + 4].try_into().unwrap();
        v.push(f32::from_le_bytes(arr));
    }
    Ok(v)
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let d = na.sqrt() * nb.sqrt();
    if d == 0.0 {
        0.0
    } else {
        dot / d
    }
}

fn content_hash(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("{:x}", h.finalize())
}

impl VectorStore {
    pub fn open(path: &Path) -> Result<Self, ScimeetError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ScimeetError::Io)?;
        }
        let conn = Connection::open(path).map_err(|e| ScimeetError::Sqlite(e.to_string()))?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                content_hash TEXT NOT NULL UNIQUE,
                doc_id TEXT NOT NULL,
                text TEXT NOT NULL,
                meta_json TEXT NOT NULL,
                embedding BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_chunks_doc ON chunks(doc_id);
            "#,
        )
        .map_err(|e| ScimeetError::Sqlite(e.to_string()))?;
        Ok(Self { conn })
    }

    pub fn upsert_chunk(
        &self,
        text: &str,
        meta: &ChunkMeta,
        embedding: &[f32],
    ) -> Result<bool, ScimeetError> {
        let h = content_hash(text);
        let exists: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM chunks WHERE content_hash = ?1",
                params![h],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| ScimeetError::Sqlite(e.to_string()))?;
        if exists.is_some() {
            return Ok(false);
        }
        let id = format!("{}_{}", meta.document_id.0, meta.chunk_index);
        let meta_json = serde_json::to_string(meta).map_err(ScimeetError::Json)?;
        let blob = f32s_to_blob(embedding);
        self.conn
            .execute(
                r#"INSERT INTO chunks (id, content_hash, doc_id, text, meta_json, embedding)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
                params![
                    id,
                    h,
                    meta.document_id.0,
                    text,
                    meta_json,
                    blob,
                ],
            )
            .map_err(|e| ScimeetError::Sqlite(e.to_string()))?;
        Ok(true)
    }

    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<IndexedChunk>, ScimeetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, text, meta_json, embedding FROM chunks")
            .map_err(|e| ScimeetError::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Vec<u8>>(3)?,
                ))
            })
            .map_err(|e| ScimeetError::Sqlite(e.to_string()))?;
        let mut scored: Vec<(f32, String, String, ChunkMeta)> = Vec::new();
        for row in rows {
            let (id, text, meta_json, emb_blob) =
                row.map_err(|e| ScimeetError::Sqlite(e.to_string()))?;
            let emb = blob_to_f32s(&emb_blob)?;
            let s = cosine(query, &emb);
            let meta: ChunkMeta = serde_json::from_str(&meta_json).map_err(ScimeetError::Json)?;
            scored.push((s, id, text, meta));
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        Ok(scored
            .into_iter()
            .map(|(score, id, text, meta)| IndexedChunk {
                id,
                text,
                meta,
                score,
            })
            .collect())
    }
}
