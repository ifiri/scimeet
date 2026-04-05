mod arxiv;
mod biorxiv;
mod pubmed;

pub use arxiv::ArxivSource;
pub use biorxiv::BiorxivMedrxivSource;
pub use pubmed::PubMedSource;

use scimeet_core::{Document, ScimeetError};

#[async_trait::async_trait]
pub trait SourceAdapter: Send + Sync {
    async fn search(&self, query: &str, max_results: usize) -> Result<Vec<Document>, ScimeetError>;
}
