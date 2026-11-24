# docula-llm

Local, embedded and remote LLM backends behind one `LlmBackend` trait, with `candle` and `tract` as the pure-Rust targets; `MockLlmBackend` returns canned JSON for tests. (`oxillama` and `oxibonsai` were removed in DOCULA-46.)

## Status

`CandleBackend` and `TractBackend` compile the schema DFA and check model paths, but do not run inference yet: they return a canned response (when set) or `{}`. Real backends are specified as DOC-1201 in docs/22.

## Key types

`LlmBackend`, `CandleBackend`, `TractBackend`, `MockLlmBackend`, `LlmError`

## Specification

- [Module 04: LLM & Local/Embedded Backends](../../docs/06_LLM_AND_LOCAL_EMBEDDED_BACKENDS.md)
- [Module 22: Finance Document Extraction & Real Inference Backends](../../docs/22_FINANCE_DOCUMENT_EXTRACTION_AND_REAL_INFERENCE.md)

## Internal dependencies

`docula-schema`

Part of the [docula](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
