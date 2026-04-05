mod config;
mod document;
mod error;

pub use config::ScimeetConfig;
pub use document::{ChunkMeta, Document, DocumentId, SourceKind};
pub use error::ScimeetError;
