use clap::{Parser, Subcommand};
use scimeet_core::ScimeetConfig;
use scimeet_index::{OllamaEmbeddings, VectorStore};
use scimeet_ingest::{chunk_document, maybe_translate_chunk};
use scimeet_rag::RagEngine;
use scimeet_sources::{ArxivSource, BiorxivMedrxivSource, PubMedSource, SourceAdapter};
use scimeet_translate::OllamaTranslator;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "scimeet")]
#[command(about = "Local research RAG (PubMed, arXiv, bioRxiv/medRxiv, Cochrane via PubMed)")]
struct Cli {
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,
    #[arg(long, default_value = "http://127.0.0.1:11434")]
    ollama: String,
    #[arg(long, default_value = "nomic-embed-text")]
    embed_model: String,
    #[arg(long, default_value = "llama3.1:8b")]
    chat_model: String,
    #[arg(long, default_value = "llama3.1:8b")]
    translate_model: String,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Ingest {
        #[arg(short, long)]
        query: String,
        #[arg(short, long, default_value = "pubmed")]
        sources: String,
        #[arg(short, long, default_value_t = 20)]
        max: usize,
    },
    Ask {
        #[arg(short, long)]
        question: String,
        #[arg(short = 'k', long, default_value_t = 5)]
        top_k: usize,
    },
}

fn cochrane_pubmed_query(user: &str) -> String {
    format!(
        "({}) AND \"Cochrane Database of Systematic Reviews\"[Journal]",
        user
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut config = ScimeetConfig::default();
    config.data_dir = cli.data_dir;
    config.ollama_base = cli.ollama;
    config.embed_model = cli.embed_model;
    config.chat_model = cli.chat_model;
    config.translate_model = cli.translate_model;
    let config = config.from_env_overrides();

    match cli.command {
        Commands::Ingest { query, sources, max } => {
            std::fs::create_dir_all(&config.data_dir)?;
            let index_path = config.index_path();
            let store = VectorStore::open(&index_path)?;
            let embeddings = OllamaEmbeddings::new(config.clone())?;
            let translator = OllamaTranslator::new(config.clone())?;
            let timeout = config.request_timeout_secs;
            let pubmed = PubMedSource::new(config.ncbi_api_key.clone(), timeout)?;
            let arxiv = ArxivSource::new(timeout)?;
            let medrxiv = BiorxivMedrxivSource::new_medrxiv(timeout)?;
            let biorxiv = BiorxivMedrxivSource::new_biorxiv(timeout)?;

            let parts: Vec<&str> = sources.split(',').map(|s| s.trim()).collect();
            let mut docs = Vec::new();
            for p in parts {
                match p {
                    "pubmed" => docs.extend(pubmed.search(&query, max).await?),
                    "arxiv" => docs.extend(arxiv.search(&query, max).await?),
                    "medrxiv" => docs.extend(medrxiv.search(&query, max).await?),
                    "biorxiv" => docs.extend(biorxiv.search(&query, max).await?),
                    "cochrane" => {
                        let q = cochrane_pubmed_query(&query);
                        docs.extend(pubmed.search(&q, max).await?)
                    }
                    _ => eprintln!("unknown source: {}", p),
                }
            }

            let mut added = 0usize;
            for doc in docs {
                let chunks = chunk_document(&doc);
                for (text, meta) in chunks {
                    let text_for_vec = maybe_translate_chunk(&config, &translator, &text).await?;
                    let emb = embeddings.embed(&text_for_vec).await?;
                    if store.upsert_chunk(&text, &meta, &emb)? {
                        added += 1;
                    }
                }
            }
            println!("ingest done, new chunks: {}", added);
        }
        Commands::Ask { question, top_k } => {
            let index_path = config.index_path();
            let store = VectorStore::open(&index_path)?;
            let engine = RagEngine::new(config.clone())?;
            let hits = engine.retrieve(&store, &question, top_k).await?;
            println!("--- sources ---");
            for h in &hits {
                println!(
                    "[{:.3}] {} | PMID:{:?} DOI:{:?}",
                    h.score, h.meta.title, h.meta.pmid, h.meta.doi
                );
            }
            println!("--- answer ---");
            let ans = engine.answer_from_hits(&question, &hits).await?;
            println!("{}", ans);
        }
    }
    Ok(())
}
