# docula-llm

Local, embedded and remote LLM backends behind one `LlmBackend` trait. Pure-Rust inference runs on `candle` and `tract`; `MockLlmBackend` returns canned JSON for tests. (`oxillama` and `oxibonsai` were removed in DOCULA-46.)

## Key types

`LlmBackend`, `CandleBackend`, `TractBackend`, `MockLlmBackend`, `LlmError`

## Specification

- [Module 04: LLM & Local/Embedded Backends](../../docs/06_LLM_AND_LOCAL_EMBEDDED_BACKENDS.md)
- [Module 22: Finance Document Extraction & Real Inference Backends](../../docs/22_FINANCE_DOCUMENT_EXTRACTION_AND_REAL_INFERENCE.md)

## Internal dependencies

`docula-schema`

Part of the [docula](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
