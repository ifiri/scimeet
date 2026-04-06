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

## Configuration

The **`scimeet-cli`** binary resolves settings in this order: **built-in defaults** (embedded TOML string in the CLI crate only) → optional **`scimeet.toml`** in the current directory, or **`--config path`** → **environment** (including a local **`.env`** file loaded at startup) → **CLI flags** (`--data-dir`, `--ollama`, …).

Library crates (`scimeet-core`, etc.) only expose `ScimeetConfig` and `from_toml_str`; they do not define defaults or read the environment. See [`.env.example`](.env.example) for variable names; copy it to `.env` to customize.

## Logging

The CLI uses [`tracing`](https://docs.rs/tracing) with `RUST_LOG` (see [`tracing-subscriber` env filter](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html)). Examples:

```bash
RUST_LOG=info ./target/release/scimeet ask --question "What trials compare SGLT2 inhibitors?"
RUST_LOG=scimeet_cli=debug,scimeet_rag=debug ./target/release/scimeet ingest --query "cancer" --sources pubmed --max 5
```

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

Read and validated only by the **`scimeet`** binary (after optional `.env`). See [`.env.example`](.env.example) for a full list.

| Variable | Purpose |
|----------|---------|
| `OLLAMA_HOST` | Ollama base URL (overrides `SCIMEET_OLLAMA_BASE` if both set) |
| `SCIMEET_OLLAMA_BASE` | Ollama base URL if `OLLAMA_HOST` unset |
| `SCIMEET_EMBED_MODEL`, `SCIMEET_CHAT_MODEL`, `SCIMEET_TRANSLATE_MODEL` | Model names |
| `SCIMEET_TRANSLATE_ON_QUERY`, `SCIMEET_TRANSLATE_ON_INGEST`, `SCIMEET_TRANSLATE_FALLBACK_TO_ORIGINAL` | `true`/`false`/`1`/`0` |
| `SCIMEET_DATA_DIR` | Data directory |
| `SCIMEET_EMBED_DIM` | Embedding width (positive integer) |
| `SCIMEET_REQUEST_TIMEOUT_SECS`, `SCIMEET_CONNECT_TIMEOUT_SECS` | HTTP timeouts (positive seconds) |
| `SCIMEET_HTTP_POOL_MAX_IDLE_PER_HOST`, `SCIMEET_HTTP_POOL_IDLE_TIMEOUT_SECS`, `SCIMEET_HTTP_USER_AGENT` | HTTP client |
| `NCBI_API_KEY` or `SCIMEET_NCBI_API_KEY` | PubMed E-utilities key |

## Limitations

- Cochrane: full text from the Cochrane Library is not available without a subscription; records available via PubMed are used.
- medRxiv/biorxiv: sampling from roughly the last ~120 days with keyword filtering from the query (see `scimeet-sources`).
- Without `--reindex`, vector search may use a flat scan until an index is built; for large tables, use `--reindex` after ingest.

## License

MIT OR Apache-2.0
