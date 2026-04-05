# scimeet

Local RAG for searching research: PubMed, arXiv, bioRxiv/medRxiv, and Cochrane reviews (via PubMed). Embeddings and answers use [Ollama](https://ollama.com/). Before embedding the query, the text is translated to English (if `whatlang` does not detect English and `translate_on_query` is enabled in the default config).

## Requirements

- Rust (edition 2021)
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
```

Comma-separated sources: `pubmed`, `arxiv`, `medrxiv`, `biorxiv`, `cochrane` (Cochrane — PubMed search scoped to the *Cochrane Database of Systematic Reviews* journal).

Data and the SQLite index default to `./data` (override with `--data-dir` or `SCIMEET_DATA_DIR`).

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

## Limitations

- Cochrane: full text from the Cochrane Library is not available without a subscription; records available via PubMed are used.
- medRxiv/biorxiv: sampling from roughly the last ~120 days with keyword filtering from the query (see `scimeet-sources`).

## License

MIT OR Apache-2.0
