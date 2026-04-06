use crate::SourceAdapter;
use async_trait::async_trait;
use reqwest::Client;
use roxmltree::Document;
use scimeet_core::{Document as SmDocument, DocumentId, ScimeetError, SourceKind};
use serde::Deserialize;

pub struct PubMedSource {
    client: Client,
    api_key: Option<String>,
}

#[derive(Deserialize)]
struct EsearchEnvelope {
    esearchresult: EsearchResult,
}

#[derive(Deserialize)]
struct EsearchResult {
    idlist: Option<Vec<String>>,
}

impl PubMedSource {
    pub fn new(client: Client, api_key: Option<String>) -> Self {
        Self { client, api_key }
    }

    async fn esearch(&self, term: &str, retmax: usize) -> Result<Vec<String>, ScimeetError> {
        let mut url = format!(
            "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pubmed&retmode=json&retmax={}&term={}",
            retmax,
            urlencoding::encode(term)
        );
        if let Some(ref k) = self.api_key {
            url.push_str("&api_key=");
            url.push_str(k);
        }
        let res = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ScimeetError::Http(e.to_string()))?;
        if !res.status().is_success() {
            return Err(ScimeetError::Http(format!("esearch {}", res.status())));
        }
        let env: EsearchEnvelope = res.json().await.map_err(|e| ScimeetError::Http(e.to_string()))?;
        Ok(env.esearchresult.idlist.unwrap_or_default())
    }

    async fn efetch(&self, ids: &[String]) -> Result<String, ScimeetError> {
        if ids.is_empty() {
            return Ok(String::new());
        }
        let idstr = ids.join(",");
        let mut url = format!(
            "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=pubmed&retmode=xml&id={}",
            idstr
        );
        if let Some(ref k) = self.api_key {
            url.push_str("&api_key=");
            url.push_str(k);
        }
        let res = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ScimeetError::Http(e.to_string()))?;
        if !res.status().is_success() {
            return Err(ScimeetError::Http(format!("efetch {}", res.status())));
        }
        res.text()
            .await
            .map_err(|e| ScimeetError::Http(e.to_string()))
    }
}

fn parse_pubmed_xml(xml: &str) -> Result<Vec<SmDocument>, ScimeetError> {
    let doc = Document::parse(xml).map_err(|e| ScimeetError::Parse(e.to_string()))?;
    let mut out = Vec::new();
    for n in doc.descendants().filter(|n| {
        n.is_element() && n.tag_name().name() == "PubmedArticle"
    }) {
        let pmid = n
            .descendants()
            .find(|x| x.is_element() && x.tag_name().name() == "PMID")
            .and_then(|x| x.text())
            .unwrap_or("")
            .trim()
            .to_string();
        if pmid.is_empty() {
            continue;
        }
        let title = n
            .descendants()
            .find(|x| x.is_element() && x.tag_name().name() == "ArticleTitle")
            .and_then(|x| x.text())
            .unwrap_or("")
            .trim()
            .to_string();
        let mut abstract_parts: Vec<String> = Vec::new();
        for abs in n
            .descendants()
            .filter(|x| x.is_element() && x.tag_name().name() == "AbstractText")
        {
            if let Some(t) = abs.text() {
                abstract_parts.push(t.trim().to_string());
            }
        }
        let abstract_text = abstract_parts.join("\n\n");
        let mut doi: Option<String> = None;
        for aid in n
            .descendants()
            .filter(|x| x.is_element() && x.tag_name().name() == "ArticleId")
        {
            if aid.attribute("IdType") == Some("doi") {
                doi = aid.text().map(|s| s.trim().to_string());
                break;
            }
        }
        let url = Some(format!("https://pubmed.ncbi.nlm.nih.gov/{}/", pmid));
        out.push(SmDocument {
            id: DocumentId(format!("pubmed:{}", pmid)),
            source: SourceKind::PubMed,
            title,
            abstract_text,
            doi,
            pmid: Some(pmid),
            url,
            published: None,
        });
    }
    Ok(out)
}

#[async_trait]
impl SourceAdapter for PubMedSource {
    async fn search(&self, query: &str, max_results: usize) -> Result<Vec<SmDocument>, ScimeetError> {
        let ids = self.esearch(query, max_results.min(200)).await?;
        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        let xml = self.efetch(&ids).await?;
        parse_pubmed_xml(&xml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scimeet_core::SourceKind;

    const SAMPLE_XML: &str = r#"<?xml version="1.0"?>
<PubmedArticleSet>
  <PubmedArticle>
    <MedlineCitation>
      <PMID Version="1">12345678</PMID>
      <Article>
        <ArticleTitle>Test Article</ArticleTitle>
        <Abstract>
          <AbstractText>First block.</AbstractText>
          <AbstractText>Second block.</AbstractText>
        </Abstract>
      </Article>
    </MedlineCitation>
    <PubmedData>
      <ArticleIdList>
        <ArticleId IdType="doi">10.1000/test.doi</ArticleId>
      </ArticleIdList>
    </PubmedData>
  </PubmedArticle>
</PubmedArticleSet>
"#;

    #[test]
    fn parse_pubmed_xml_extracts_article() {
        let docs = parse_pubmed_xml(SAMPLE_XML).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].id.0, "pubmed:12345678");
        assert_eq!(docs[0].source, SourceKind::PubMed);
        assert_eq!(docs[0].title, "Test Article");
        assert_eq!(docs[0].abstract_text, "First block.\n\nSecond block.");
        assert_eq!(docs[0].pmid.as_deref(), Some("12345678"));
        assert_eq!(docs[0].doi.as_deref(), Some("10.1000/test.doi"));
        assert_eq!(
            docs[0].url.as_deref(),
            Some("https://pubmed.ncbi.nlm.nih.gov/12345678/")
        );
    }
}
