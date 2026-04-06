mod config;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::{load_dotenv, load_scimeet_config};
use scimeet_core::{build_http_client, ChunkMeta, ScimeetConfig};
use scimeet_index::{OllamaEmbeddings, VectorStore};
use scimeet_ingest::{chunk_document, maybe_translate_chunk};
use scimeet_rag::RagEngine;
use scimeet_sources::{ArxivSource, BiorxivMedrxivSource, PubMedSource, SourceAdapter};
use scimeet_translate::OllamaTranslator;
use std::path::PathBuf;
use tracing::{info, instrument};

const INGEST_BATCH: usize = 32;

#[derive(Parser)]
#[command(name = "scimeet")]
#[command(about = "Local research RAG (PubMed, arXiv, bioRxiv/medRxiv, Cochrane via PubMed)")]
struct Cli {
    #[command(flatten)]
    global: GlobalOpts,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser)]
struct GlobalOpts {
    #[arg(long, global = true, help = "Path to scimeet.toml (optional; else ./scimeet.toml if present, else built-in defaults)")]
    config: Option<PathBuf>,
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    ollama: Option<String>,
    #[arg(long, global = true)]
    embed_model: Option<String>,
    #[arg(long, global = true)]
    embed_dim: Option<usize>,
    #[arg(long, global = true)]
    chat_model: Option<String>,
    #[arg(long, global = true)]
    translate_model: Option<String>,
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

fn merge_global(config: &mut ScimeetConfig, g: &GlobalOpts) {
    if let Some(ref p) = g.data_dir {
        config.data_dir = p.clone();
    }
    if let Some(ref s) = g.ollama {
        config.ollama_base = s.clone();
    }
    if let Some(ref s) = g.embed_model {
        config.embed_model = s.clone();
    }
    if let Some(n) = g.embed_dim {
        config.embed_dim = n;
    }
    if let Some(ref s) = g.chat_model {
        config.chat_model = s.clone();
    }
    if let Some(ref s) = g.translate_model {
        config.translate_model = s.clone();
    }
}

fn cochrane_pubmed_query(user: &str) -> String {
    format!(
        "({}) AND \"Cochrane Database of Systematic Reviews\"[Journal]",
        user
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let mut config = load_scimeet_config(cli.global.config.as_deref())?;
    merge_global(&mut config, &cli.global);
    info!(data_dir = ?config.data_dir, ollama = %config.ollama_base, "config");

    match cli.command {
        Commands::Ingest {
            query,
            sources,
            max,
            reindex,
        } => {
            run_ingest(config, query, sources, max, reindex).await?;
        }
        Commands::Ask { question, top_k } => {
            run_ask(config, question, top_k).await?;
        }
    }
    Ok(())
}

#[instrument(skip(config))]
async fn run_ingest(
    config: ScimeetConfig,
    query: String,
    sources: String,
    max: usize,
    reindex: bool,
) -> Result<()> {
    std::fs::create_dir_all(&config.data_dir)?;
    let http = build_http_client(&config)?;
    let index_path = config.index_path();
    let store = VectorStore::open(&index_path, config.embed_dim).await?;
    let embeddings = OllamaEmbeddings::new(config.clone(), http.clone());
    let translator = OllamaTranslator::new(config.clone(), http.clone());
    let pubmed = PubMedSource::new(http.clone(), config.ncbi_api_key.clone());
    let arxiv = ArxivSource::new(http.clone());
    let medrxiv = BiorxivMedrxivSource::new_medrxiv(http.clone());
    let biorxiv = BiorxivMedrxivSource::new_biorxiv(http);

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
            _ => tracing::warn!(source = p, "unknown source"),
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
    Ok(())
}

#[instrument(skip(config))]
async fn run_ask(
    config: ScimeetConfig,
    question: String,
    top_k: usize,
) -> Result<()> {
    let http = build_http_client(&config)?;
    let index_path = config.index_path();
    let store = VectorStore::open(&index_path, config.embed_dim).await?;
    let engine = RagEngine::new(config, http);
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
    Ok(())
}
