# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Removed (breaking)
- Removed `oxirag`, `oxillama` and `oxibonsai` dependencies and backends following their licence change to BUSL-1.1 and repository privatization (2026-11). Use pure-Rust `CandleBackend` (`candle`), `TractBackend` (`tract`), and `embedag_rag::VerifiedRagEngine` (via `EmbedagGraphRetriever`) instead.
