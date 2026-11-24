# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Security
- Cleared every `cargo audit` advisory: `zip` 8.6 is gone (`candle-core` 0.11.1 and `office_oxide` 0.1.17 use `zip` 9), `docula-ingest` reads office files with `zip` 9, `rustls`, `tokio` and related crates are on patched releases, and Diesel is 2.4 so `libsqlite3-sys` resolves to a fixed release. The repository no longer carries an advisory ignore list.

### Changed
- `docula-rag` builds its `DocumentKnowledgeGraph` on `oxrdf` instead of `oxirs-core`, which pinned vulnerable `oxitls` and `oxrdfxml` versions. SPARQL and persistence remain planned.
- `docula-hitl` uses Diesel 2.4 through a `diesel-cte-ext` whose `sqlite` and `postgres` features no longer pull `diesel-async`.
- CI runs the full `--all-features` test suite on Linux, macOS and Windows; libpq and sqlite3 are installed on the macOS and Windows runners.

### Removed (breaking)
- Removed `oxirag`, `oxillama` and `oxibonsai` dependencies and backends following their licence change to BUSL-1.1 and repository privatization (2026-11). Use pure-Rust `CandleBackend` (`candle`), `TractBackend` (`tract`), and `embedag_rag::VerifiedRagEngine` (via `EmbedagGraphRetriever`) instead.
