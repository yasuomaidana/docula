# docula-cli

Terminal front end for the engine.

## Commands

```bash
cargo run -p docula-cli -- extract --input invoice.pdf --dpi 150
cargo run -p docula-cli -- inspect --input invoice.pdf
cargo run -p docula-cli -- codes   --input invoice.png
```

- `extract` runs the full pipeline (flags: `--dpi`, `--no-redaction`, `--no-codes`) and prints JSON.
- `inspect` reports PDF structure and whether each page is born-digital or scanned.
- `codes` lists the QR codes and barcodes found, including financial payloads.

## Specification

- [Module 01: Ingestion & Intelligent Routing](../../docs/03_INGESTION_AND_ROUTING.md)
- [Module 17: Pure-Rust Optical Code Extraction (QR & Barcodes)](../../docs/17_OPTICAL_CODE_EXTRACTION.md)

## Internal dependencies

`docula` (facade)

Part of the [docula](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
