use crate::SourceAdapter;
use async_trait::async_trait;
use reqwest::Client;
use roxmltree::Document;
use scimeet_core::{Document as SmDocument, DocumentId, ScimeetError, SourceKind};

pub struct ArxivSource {
    client: Client,
}

impl ArxivSource {
    pub fn new(client: Client) -> Self {
        Self { client }
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

#[cfg(test)]
mod tests {
    use super::*;
    use scimeet_core::SourceKind;

    const SAMPLE_ATOM: &str = r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <id>http://arxiv.org/abs/1234.5678v1</id>
    <title>  Sample Paper Title  </title>
    <summary>Abstract line one.</summary>
    <published>2021-06-01T00:00:00Z</published>
  </entry>
</feed>
"#;

    #[test]
    fn parse_arxiv_atom_extracts_entry() {
        let docs = parse_arxiv_atom(SAMPLE_ATOM).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].id.0, "arxiv:1234.5678v1");
        assert_eq!(docs[0].source, SourceKind::Arxiv);
        assert_eq!(docs[0].title, "Sample Paper Title");
        assert_eq!(docs[0].abstract_text, "Abstract line one.");
        assert_eq!(
            docs[0].url.as_deref(),
            Some("http://arxiv.org/abs/1234.5678v1")
        );
        assert_eq!(
            docs[0].published.as_deref(),
            Some("2021-06-01T00:00:00Z")
        );
    }
}
