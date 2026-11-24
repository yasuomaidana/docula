# docula-ingest

PDF object-tree inspection, font `ToUnicode` validation and the routing heuristics that decide whether a page is born-digital (text can be read directly) or scanned (needs rasterization and OCR).

## Key types

`PdfInspector`, `DocumentRouter`, `RouterConfig`, `PageRoute`, `DocumentRoutingReport`, `RasterJob`

## Specification

- [Module 01: Ingestion & Intelligent Routing](../../docs/03_INGESTION_AND_ROUTING.md)
- [Module 19: Pure-Rust Ofimatic & Office Suite Ingestion Engine](../../docs/19_OFIMATIC_OFFICE_SUITE_INGESTION.md)

## Internal dependencies

`docula-core`

Part of the [docula](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
