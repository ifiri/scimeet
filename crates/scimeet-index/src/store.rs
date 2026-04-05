use arrow_array::cast::AsArray;
use arrow_array::types::Float32Type;
use arrow_array::ArrayRef;
use arrow_array::{FixedSizeListArray, Float32Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use futures::TryStreamExt;
use lancedb::index::Index;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::{connect, DistanceType, Table};
use scimeet_core::{ChunkMeta, ScimeetError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;

pub struct VectorStore {
    table: Arc<Table>,
    dim: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexedChunk {
    pub id: String,
    pub text: String,
    pub meta: ChunkMeta,
    pub score: f32,
}

const TABLE_NAME: &str = "chunks";

fn content_hash(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("{:x}", h.finalize())
}

fn chunk_schema(dim: usize) -> Arc<Schema> {
    let vec_item = Field::new("item", DataType::Float32, true);
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("content_hash", DataType::Utf8, false),
        Field::new("doc_id", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new("meta_json", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(Arc::new(vec_item), dim as i32),
            false,
        ),
    ]))
}

fn record_batch_from_row(
    id: &str,
    hash: &str,
    doc_id: &str,
    text: &str,
    meta_json: &str,
    embedding: &[f32],
    dim: usize,
) -> Result<RecordBatch, ScimeetError> {
    if embedding.len() != dim {
        return Err(ScimeetError::Parse(format!(
            "embedding dim {} != expected {}",
            embedding.len(),
            dim
        )));
    }
    let flat: ArrayRef = Arc::new(Float32Array::from_iter_values(
        embedding.iter().copied(),
    ));
    let list = FixedSizeListArray::try_new(
        Arc::new(Field::new("item", DataType::Float32, true)),
        dim as i32,
        flat,
        None,
    )
    .map_err(|e| ScimeetError::LanceDb(e.to_string()))?;
    let schema = chunk_schema(dim);
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec![id.to_string()])),
            Arc::new(StringArray::from(vec![hash.to_string()])),
            Arc::new(StringArray::from(vec![doc_id.to_string()])),
            Arc::new(StringArray::from(vec![text.to_string()])),
            Arc::new(StringArray::from(vec![meta_json.to_string()])),
            Arc::new(list),
        ],
    )
    .map_err(|e| ScimeetError::LanceDb(e.to_string()))
}

fn record_batch_from_rows(
    rows: Vec<(String, String, String, String, String, Vec<f32>)>,
    dim: usize,
) -> Result<RecordBatch, ScimeetError> {
    let n = rows.len();
    let mut flat: Vec<f32> = Vec::with_capacity(n * dim);
    for r in &rows {
        if r.5.len() != dim {
            return Err(ScimeetError::Parse(format!(
                "embedding dim {} != expected {}",
                r.5.len(),
                dim
            )));
        }
        flat.extend_from_slice(&r.5);
    }
    let flat_arr: ArrayRef = Arc::new(Float32Array::from(flat));
    let list = FixedSizeListArray::try_new(
        Arc::new(Field::new("item", DataType::Float32, true)),
        dim as i32,
        flat_arr,
        None,
    )
    .map_err(|e| ScimeetError::LanceDb(e.to_string()))?;
    let schema = chunk_schema(dim);
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from_iter(
                rows.iter().map(|r| Some(r.0.clone())),
            )),
            Arc::new(StringArray::from_iter(
                rows.iter().map(|r| Some(r.1.clone())),
            )),
            Arc::new(StringArray::from_iter(
                rows.iter().map(|r| Some(r.2.clone())),
            )),
            Arc::new(StringArray::from_iter(
                rows.iter().map(|r| Some(r.3.clone())),
            )),
            Arc::new(StringArray::from_iter(
                rows.iter().map(|r| Some(r.4.clone())),
            )),
            Arc::new(list),
        ],
    )
    .map_err(|e| ScimeetError::LanceDb(e.to_string()))
}

fn map_err(e: impl std::fmt::Display) -> ScimeetError {
    ScimeetError::LanceDb(e.to_string())
}

impl VectorStore {
    pub async fn open(path: &Path, dim: usize) -> Result<Self, ScimeetError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ScimeetError::Io)?;
        }
        std::fs::create_dir_all(path).map_err(ScimeetError::Io)?;
        let uri = path.to_string_lossy();
        let conn = connect(uri.as_ref())
            .execute()
            .await
            .map_err(map_err)?;
        let names = conn
            .table_names()
            .execute()
            .await
            .map_err(map_err)?;
        let table = if names.iter().any(|n| n == TABLE_NAME) {
            conn.open_table(TABLE_NAME)
                .execute()
                .await
                .map_err(map_err)?
        } else {
            let schema = chunk_schema(dim);
            conn.create_empty_table(TABLE_NAME, schema)
                .execute()
                .await
                .map_err(map_err)?
        };
        Ok(Self {
            table: Arc::new(table),
            dim,
        })
    }

    pub async fn upsert_chunk(
        &self,
        text: &str,
        meta: &ChunkMeta,
        embedding: &[f32],
    ) -> Result<bool, ScimeetError> {
        let h = content_hash(text);
        let existing = self
            .table
            .query()
            .only_if(format!("content_hash = '{h}'"))
            .limit(1)
            .execute()
            .await
            .map_err(map_err)?
            .try_collect::<Vec<_>>()
            .await
            .map_err(map_err)?;
        if !existing.is_empty() && existing.iter().any(|b| b.num_rows() > 0) {
            return Ok(false);
        }
        let id = format!("{}_{}", meta.document_id.0, meta.chunk_index);
        let meta_json = serde_json::to_string(meta).map_err(ScimeetError::Json)?;
        let batch = record_batch_from_row(
            &id,
            &h,
            &meta.document_id.0,
            text,
            &meta_json,
            embedding,
            self.dim,
        )?;
        self.table
            .add(vec![batch])
            .execute()
            .await
            .map_err(map_err)?;
        Ok(true)
    }

    pub async fn upsert_chunks_batch(
        &self,
        items: Vec<(String, ChunkMeta, Vec<f32>)>,
    ) -> Result<usize, ScimeetError> {
        let mut rows: Vec<(String, String, String, String, String, Vec<f32>)> = Vec::new();
        for (text, meta, emb) in items {
            let h = content_hash(&text);
            let existing = self
                .table
                .query()
                .only_if(format!("content_hash = '{h}'"))
                .limit(1)
                .execute()
                .await
                .map_err(map_err)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(map_err)?;
            if !existing.is_empty() && existing.iter().any(|b| b.num_rows() > 0) {
                continue;
            }
            let id = format!("{}_{}", meta.document_id.0, meta.chunk_index);
            let meta_json = serde_json::to_string(&meta).map_err(ScimeetError::Json)?;
            rows.push((
                id,
                h,
                meta.document_id.0,
                text,
                meta_json,
                emb,
            ));
        }
        if rows.is_empty() {
            return Ok(0);
        }
        let batch = record_batch_from_rows(rows, self.dim)?;
        let n = batch.num_rows();
        self.table
            .add(vec![batch])
            .execute()
            .await
            .map_err(map_err)?;
        Ok(n)
    }

    pub async fn search(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<IndexedChunk>, ScimeetError> {
        if query.len() != self.dim {
            return Err(ScimeetError::Parse(format!(
                "query embedding dim {} != expected {}",
                query.len(),
                self.dim
            )));
        }
        let batches = self
            .table
            .query()
            .nearest_to(query)
            .map_err(map_err)?
            .distance_type(DistanceType::Cosine)
            .limit(k)
            .execute()
            .await
            .map_err(map_err)?
            .try_collect::<Vec<_>>()
            .await
            .map_err(map_err)?;
        let mut out: Vec<IndexedChunk> = Vec::new();
        for batch in batches {
            let id_arr = batch
                .column_by_name("id")
                .ok_or_else(|| ScimeetError::Parse("missing id".to_string()))?
                .as_string::<i32>();
            let text_arr = batch
                .column_by_name("text")
                .ok_or_else(|| ScimeetError::Parse("missing text".to_string()))?
                .as_string::<i32>();
            let meta_arr = batch
                .column_by_name("meta_json")
                .ok_or_else(|| ScimeetError::Parse("missing meta_json".to_string()))?
                .as_string::<i32>();
            let dist_arr = batch
                .column_by_name("_distance")
                .ok_or_else(|| ScimeetError::Parse("missing _distance".to_string()))?
                .as_primitive::<Float32Type>();
            for i in 0..batch.num_rows() {
                let dist = dist_arr.value(i);
                let score = (1.0 - dist).max(0.0);
                let meta: ChunkMeta = serde_json::from_str(meta_arr.value(i))
                    .map_err(ScimeetError::Json)?;
                out.push(IndexedChunk {
                    id: id_arr.value(i).to_string(),
                    text: text_arr.value(i).to_string(),
                    meta,
                    score,
                });
            }
        }
        Ok(out)
    }

    pub async fn create_vector_index(&self) -> Result<(), ScimeetError> {
        self.table
            .create_index(&["vector"], Index::Auto)
            .execute()
            .await
            .map_err(map_err)?;
        Ok(())
    }

    pub fn dim(&self) -> usize {
        self.dim
    }
}
