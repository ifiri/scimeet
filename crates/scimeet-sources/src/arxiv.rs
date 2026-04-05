use crate::SourceAdapter;
use async_trait::async_trait;
use reqwest::Client;
use roxmltree::Document;
use scimeet_core::{Document as SmDocument, DocumentId, ScimeetError, SourceKind};

pub struct ArxivSource {
    client: Client,
}

impl ArxivSource {
    pub fn new(timeout_secs: u64) -> Result<Self, ScimeetError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .user_agent("scimeet/0.1")
            .build()
            .map_err(|e| ScimeetError::Http(e.to_string()))?;
        Ok(Self { client })
    }
}

fn parse_arxiv_atom(xml: &str) -> Result<Vec<SmDocument>, ScimeetError> {
    let doc = Document::parse(xml).map_err(|e| ScimeetError::Parse(e.to_string()))?;
    let mut out = Vec::new();
    for entry in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "entry")
    {
        let id = entry
            .descendants()
            .find(|x| x.is_element() && x.tag_name().name() == "id")
            .and_then(|x| x.text())
            .unwrap_or("")
            .trim()
            .to_string();
        let title = entry
            .descendants()
            .find(|x| x.is_element() && x.tag_name().name() == "title")
            .and_then(|x| x.text())
            .unwrap_or("")
            .trim()
            .replace('\n', " ");
        let summary = entry
            .descendants()
            .find(|x| x.is_element() && x.tag_name().name() == "summary")
            .and_then(|x| x.text())
            .unwrap_or("")
            .trim()
            .to_string();
        let short_id = id
            .rsplit('/')
            .next()
            .unwrap_or(&id)
            .to_string();
        out.push(SmDocument {
            id: DocumentId(format!("arxiv:{}", short_id)),
            source: SourceKind::Arxiv,
            title,
            abstract_text: summary,
            doi: None,
            pmid: None,
            url: Some(id),
            published: entry
                .descendants()
                .find(|x| x.is_element() && x.tag_name().name() == "published")
                .and_then(|x| x.text())
                .map(|s| s.trim().to_string()),
        });
    }
    Ok(out)
}

#[async_trait]
impl SourceAdapter for ArxivSource {
    async fn search(&self, query: &str, max_results: usize) -> Result<Vec<SmDocument>, ScimeetError> {
        let q = format!("all:{}", query);
        let url = format!(
            "http://export.arxiv.org/api/query?search_query={}&max_results={}&sortBy=relevance&sortOrder=descending",
            urlencoding::encode(&q),
            max_results.min(100)
        );
        let res = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ScimeetError::Http(e.to_string()))?;
        if !res.status().is_success() {
            return Err(ScimeetError::Http(format!("arxiv {}", res.status())));
        }
        let xml = res.text().await.map_err(|e| ScimeetError::Http(e.to_string()))?;
        parse_arxiv_atom(&xml)
    }
}
