use crate::SourceAdapter;
use async_trait::async_trait;
use chrono::{Duration, Utc};
use reqwest::Client;
use scimeet_core::{Document as SmDocument, DocumentId, ScimeetError, SourceKind};
use serde::Deserialize;

pub struct BiorxivMedrxivSource {
    client: Client,
    server: String,
}

#[derive(Deserialize)]
struct DetailsEnvelope {
    #[serde(default)]
    collection: Vec<BiorxivItem>,
}

#[derive(Deserialize)]
struct BiorxivItem {
    title: String,
    #[serde(rename = "abstract")]
    abstract_text: String,
    doi: String,
    date: String,
}

impl BiorxivMedrxivSource {
    pub fn new_medrxiv(timeout_secs: u64) -> Result<Self, ScimeetError> {
        Self::with_server("medrxiv", timeout_secs)
    }

    pub fn new_biorxiv(timeout_secs: u64) -> Result<Self, ScimeetError> {
        Self::with_server("biorxiv", timeout_secs)
    }

    fn with_server(server: &str, timeout_secs: u64) -> Result<Self, ScimeetError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| ScimeetError::Http(e.to_string()))?;
        Ok(Self {
            client,
            server: server.to_string(),
        })
    }

    fn matches_query(text: &str, query: &str) -> bool {
        let t = text.to_lowercase();
        for w in query.split_whitespace() {
            if w.len() > 2 && !t.contains(&w.to_lowercase()) {
                return false;
            }
        }
        true
    }
}

#[async_trait]
impl SourceAdapter for BiorxivMedrxivSource {
    async fn search(&self, query: &str, max_results: usize) -> Result<Vec<SmDocument>, ScimeetError> {
        let end = Utc::now().date_naive();
        let start = end - Duration::days(120);
        let url = format!(
            "https://api.biorxiv.org/details/{}/{}/{}/0",
            self.server,
            start.format("%Y-%m-%d"),
            end.format("%Y-%m-%d")
        );
        let res = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ScimeetError::Http(e.to_string()))?;
        if !res.status().is_success() {
            return Err(ScimeetError::Http(format!("biorxiv api {}", res.status())));
        }
        let env: DetailsEnvelope = res.json().await.map_err(|e| ScimeetError::Http(e.to_string()))?;
        let mut out: Vec<SmDocument> = Vec::new();
        for item in env.collection {
            let blob = format!("{} {}", item.title, item.abstract_text);
            if !Self::matches_query(&blob, query) {
                continue;
            }
            let source_kind = if self.server == "medrxiv" {
                SourceKind::MedRxiv
            } else {
                SourceKind::BioRxiv
            };
            let url = Some(format!("https://doi.org/{}", item.doi));
            out.push(SmDocument {
                id: DocumentId(format!("{}:{}", self.server, item.doi)),
                source: source_kind,
                title: item.title,
                abstract_text: item.abstract_text,
                doi: Some(item.doi),
                pmid: None,
                url,
                published: Some(item.date),
            });
            if out.len() >= max_results {
                break;
            }
        }
        Ok(out)
    }
}
