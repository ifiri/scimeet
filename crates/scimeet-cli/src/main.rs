use clap::{Parser, Subcommand};
use scimeet_core::{ChunkMeta, ScimeetConfig};
use scimeet_index::{OllamaEmbeddings, VectorStore};
use scimeet_ingest::{chunk_document, maybe_translate_chunk};
use scimeet_rag::RagEngine;
use scimeet_sources::{ArxivSource, BiorxivMedrxivSource, PubMedSource, SourceAdapter};
use scimeet_translate::OllamaTranslator;
use std::path::PathBuf;

const INGEST_BATCH: usize = 32;

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
    #[arg(long, default_value_t = 768)]
    embed_dim: usize,
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
        #[arg(long, default_value_t = false)]
        reindex: bool,
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
    config.embed_dim = cli.embed_dim;
    config.chat_model = cli.chat_model;
    config.translate_model = cli.translate_model;
    let config = config.from_env_overrides();

    match cli.command {
        Commands::Ingest {
            query,
            sources,
            max,
            reindex,
        } => {
            std::fs::create_dir_all(&config.data_dir)?;
            let index_path = config.index_path();
            let store = VectorStore::open(&index_path, config.embed_dim).await?;
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
            let mut pending: Vec<(String, ChunkMeta, Vec<f32>)> = Vec::new();
            for doc in docs {
                let chunks = chunk_document(&doc);
                for (text, meta) in chunks {
                    let text_for_vec = maybe_translate_chunk(&config, &translator, &text).await?;
                    let emb = embeddings.embed(&text_for_vec).await?;
                    pending.push((text, meta, emb));
                    if pending.len() >= INGEST_BATCH {
                        added += store.upsert_chunks_batch(std::mem::take(&mut pending)).await?;
                    }
                }
            }
            if !pending.is_empty() {
                added += store.upsert_chunks_batch(pending).await?;
            }
            if reindex && added > 0 {
                store.create_vector_index().await?;
            }
            println!("ingest done, new chunks: {}", added);
        }
        Commands::Ask { question, top_k } => {
            let index_path = config.index_path();
            let store = VectorStore::open(&index_path, config.embed_dim).await?;
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
