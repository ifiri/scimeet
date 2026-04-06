use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScimeetError {
    #[error("http: {0}")]
    Http(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("lancedb: {0}")]
    LanceDb(String),
    #[error("config: {0}")]
    Config(String),
    #[error("ollama: {0}")]
    Ollama(String),
    #[error("parse: {0}")]
    Parse(String),
    #[error("{0}")]
    Other(String),
}
