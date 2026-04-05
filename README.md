# scimeet

Local RAG for searching research: PubMed, arXiv, bioRxiv/medRxiv, and Cochrane reviews (via PubMed). Embeddings and answers use [Ollama](https://ollama.com/). Before embedding the query, the text is translated to English (if `whatlang` does not detect English and `translate_on_query` is enabled in the default config).

## Requirements

- Rust **1.91+** (required by [LanceDB](https://github.com/lancedb/lancedb) Rust crate; edition 2021)
- Ollama with models, for example:
  - `ollama pull nomic-embed-text`
  - `ollama pull llama3.1:8b` (or another chat model; the same one can be used for translation)

Optional: `NCBI_API_KEY` for higher [NCBI E-utilities](https://ncbiinsights.ncbi.nlm.nih.gov/2017/11/02/new-api-keys-for-the-e-utilities/) rate limits.

## Setup

```bash
ollama pull nomic-embed-text
ollama pull llama3.1:8b
```

## Build

```bash
cargo build --release
```

Binary: `target/release/scimeet` (or `target\release\scimeet.exe` on Windows).

## Ingestion

```bash
./target/release/scimeet ingest --query "diabetes mellitus" --sources pubmed,arxiv --max 20
./target/release/scimeet ingest --query "diabetes mellitus" --sources pubmed --max 20 --reindex
```

Comma-separated sources: `pubmed`, `arxiv`, `medrxiv`, `biorxiv`, `cochrane` (Cochrane — PubMed search scoped to the *Cochrane Database of Systematic Reviews* journal).

Vector index (LanceDB) defaults to `./data/lancedb` under the data directory (override with `--data-dir` or `SCIMEET_DATA_DIR`). The previous SQLite file `data/index.sqlite` is **not** migrated automatically; run `ingest` again to rebuild the index.

Embedding width must match the model (default **768** for `nomic-embed-text`). Set `--embed-dim` or `SCIMEET_EMBED_DIM` if you use another model.

Use `ingest --reindex` to build an ANN vector index after adding chunks (optional; improves search at scale).

## Questions

```bash
./target/release/scimeet ask --question "What trials compare SGLT2 inhibitors?" --top-k 5
```

Retrieved chunks (score, PMID, DOI) are printed first, then the model answer.

## Environment variables

| Variable | Purpose |
|----------|---------|
| `OLLAMA_HOST` | Ollama base URL (default `http://127.0.0.1:11434`) |
| `NCBI_API_KEY` | NCBI key for E-utilities |
| `SCIMEET_DATA_DIR` | Data directory instead of `./data` |
| `SCIMEET_EMBED_DIM` | Embedding dimension (default `768`) |

## Limitations

- Cochrane: full text from the Cochrane Library is not available without a subscription; records available via PubMed are used.
- medRxiv/biorxiv: sampling from roughly the last ~120 days with keyword filtering from the query (see `scimeet-sources`).
- Without `--reindex`, vector search may use a flat scan until an index is built; for large tables, use `--reindex` after ingest.

## License

MIT OR Apache-2.0
