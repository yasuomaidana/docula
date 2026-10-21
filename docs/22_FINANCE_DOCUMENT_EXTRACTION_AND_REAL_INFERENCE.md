# Module 22: Finance Document Extraction & Real Inference Backends

## 1. Purpose

This module turns docula's scaffolded inference paths into working ones, and adds the finance-document capabilities needed by personal-finance consumers: Mexican CFDI 4.0 invoices, bank statements, and thermal receipts.

Requested by: Kakebo (Plane `KAKEBODM`; specs in `kakebo-specs`, `data_modeling/architecture_review_2026-10.md` and `second_assessment_2026-10.md`).

## 2. Assessment of the Current Implementation

1. **The spec and the code have drifted.** `05_OCR_AND_SPATIAL_EXTRACTION.md` specifies `rusto-rs` (PP-OCRv6) and `ocrs`. The code instead uses Otsu thresholding plus connected components (`DbNetDetector`) and a recognizer that returns pre-registered strings (`crnn.rs:76-110`). DEV_STATUS marks milestones 1–10 as verified.
2. **The LLM backends are stubs.** `OxiLlamaBackend::generate_constrained` ignores its prompts and returns canned output or `"{}"`. The `remote-ollama`, `remote-openai` and `local-onnx` features are empty.
3. **Money is `f64`** in `ReceiptData`, `InvoiceData` and `BankStatementData`, and the validators compare with an epsilon.
4. **CFDI support covers the QR URL only** (`MexicanSatCfdiPayload`). The `cfdi:Comprobante` XML is not parsed, even though `quick-xml` is already a dependency.
5. **Portability:** `docula-rag` depends on `../../../embedag` through a relative path, and there is no mobile or wasm profile.
6. **Board hygiene:** Plane lists Milestones 9, 10 and 11 twice, with and without the crate suffix.
7. **What already works and is valuable:** lopdf text extraction, CSV/XLSX/DOCX ingestion, rxing QR decoding with SEPA/CFDI-QR parsers, the domain validators, deskew/binarize filters, and BM25.

## 3. Requirements (Milestone 12: Finance Document Extraction & Real Inference Backends)

| Ticket | Priority | Points | Crate | Requirement |
|---|---|---|---|---|
| `DOC-1201` | urgent | 5 | `docula-llm` | Real HTTP vision LlmBackend (Anthropic / OpenAI / Ollama-compatible) |
| `DOC-1202` | high | 8 | `docula-ocr` | Wire the specified neural OCR backends (rusto-rs PP-OCRv6 primary, ocrs optional) into docula-ocr |
| `DOC-1203` | high | 5 | `docula-domains` | CFDI 4.0 XML parser + RFC validation |
| `DOC-1204` | high | 5 | `docula-ingest` | Mexican bank statement CSV profiles + OFX/QFX import |
| `DOC-1205` | high | 3 | `docula-domains` | Integer fixed-point money mode (i64 × 10⁴) instead of f64 in domain types |
| `DOC-1206` | medium | 3 | `docula` | Per-field bounding box, page and confidence in ExtractionResult |
| `DOC-1207` | medium | 5 | `docula-domains` | Deterministic line-item parser for Mexican thermal receipts |
| `DOC-1208` | medium | 2 | `docula-rag` | Replace relative `../../../embedag` path dependency with optional git/workspace dependency |
| `DOC-1209` | medium | 3 | `docula` | Mobile/wasm-friendly feature profile |
| `DOC-1210` | medium | 2 | `docula` | Reconcile DEV_STATUS and specs with implementation (OCR, LLM backends) + deduplicate Plane milestones |

## 4. Specifications

### DOC-1201: Real HTTP vision LlmBackend (Anthropic / OpenAI / Ollama-compatible)

Every current `LlmBackend` returns canned output or `"{}"` (`docula-llm/src/oxillama.rs`), and the `remote-ollama`, `remote-openai` and `local-onnx` features are empty.

**Request:** a real HTTP backend, behind a `remote-http` feature, that sends page images plus the schema-constrained prompt to a multimodal model and returns `generate_constrained` JSON. It should be validated against the `schemars` schema and retry once if the output doesn't validate.

**Acceptance criteria:**
- An env/config-driven endpoint and model.
- Supports image input (base64).
- Timeouts and cost or token usage are reported in the result.
- Uses no `tokio/full`; a minimal `reqwest` with rustls.
- An integration test runs against a recorded fixture.

*Unblocks Kakebo KAK-406.*

### DOC-1202: Wire the specified neural OCR backends (rusto-rs PP-OCRv6 primary, ocrs optional) into docula-ocr

`docs/05_OCR_AND_SPATIAL_EXTRACTION.md` §2 specifies `rusto-rs` (pure-Rust RapidOCR / PP-OCRv6 on MNN) as the primary engine and `ocrs` (RTen) as an optional one. Neither is a dependency today:
- `DbNetDetector` is Otsu thresholding plus connected components.
- `CrnnRecognizer::recognize_crop` returns pre-registered strings (`crnn.rs:76-110`).

**Request:** implement the backends the spec already names, keeping the zero-FFI stack, and return `SpatialSpan`s with per-word confidence. It must handle Spanish text with accents.

**Acceptance criteria:**
- Character error rate (CER) below 5% on a fixture set of Mexican receipts.
- Model weights are fetched or loaded from a path and never vendored into git.
- `MockOcrEngine` is kept for tests.
- DEV_STATUS reflects what is real.

*Unblocks Kakebo KAK-406.*

### DOC-1203: CFDI 4.0 XML parser + RFC validation

Today only the SAT verification QR URL is parsed (`MexicanSatCfdiPayload`).

**Request:** use quick-xml, which is already a dependency, to parse `cfdi:Comprobante` (v3.3 and 4.0) into a typed `CfdiInvoice`. It needs these parts:
- Emisor and Receptor: RFC, name, RegimenFiscal, UsoCFDI.
- Conceptos: ClaveProdServ, quantity, unit value, amount, discount, and per-concept Impuestos (Traslados and Retenciones).
- Totals: SubTotal, Descuento, Total, Moneda, TipoCambio, FormaPago, MetodoPago.
- `tfd:TimbreFiscalDigital`: UUID and FechaTimbrado.

It should also validate the RFC format (persona física 13 characters, moral 12) and the check digit. Amounts must be exact decimals (see DOC-1205).

**Acceptance criteria:** tests against real-shaped fixtures (ingreso, egreso, pago complemento).

*Unblocks Kakebo KAK-204 / KAK-404.*

### DOC-1204: Mexican bank statement CSV profiles + OFX/QFX import

`SpreadsheetReader::read_csv` returns raw cells.

**Request:** declarative `BankCsvProfile`s for BBVA MX, Banorte, Santander MX and Nu MX. Each profile maps the date, descriptor, debit and credit or signed amount, and balance columns. It needs locale-aware parsing (`dd/mm/yyyy`, `1,234.56` vs `1.234,56`) and auto-detection of the profile from the header row. Also add an OFX/QFX parser that produces `BankStatementData`.

**Acceptance criteria:**
- `validate_running_balances` passes on fixtures.
- Unknown CSVs fall back to column inference with a confidence score.

*Unblocks Kakebo KAK-405.*

### DOC-1205: Integer fixed-point money mode (i64 × 10⁴) instead of f64 in domain types

`ReceiptData`, `InvoiceData` and `BankStatementData` use `f64` for money. Kakebo stores money as `i64` scaled by 10⁴ and uses banker's rounding.

**Request:** a `money-fixed` feature, or a generic `Amount` type, that makes the domain structs carry `i64 × 10⁴` (or `rust_decimal`). The validators must compare exactly, with no epsilon. The default stays backward compatible.

**Acceptance criteria:** receipt totals equal the sum of their items exactly on fixtures.

*Unblocks Kakebo KAK-203.*

### DOC-1206: Per-field bounding box, page and confidence in ExtractionResult

Kakebo's `extraction_fields` table highlights each extracted value on the image and records user corrections.

**Request:** have `ExtractionResult<T>` expose `fields: Vec<FieldProvenance { json_path, value, page, bbox (normalized ×10⁴), confidence_bps, source ocr|llm|parser }>`, tracing each field back to the OCR spans it came from.

**Acceptance criteria:** every leaf field in `ReceiptData` gets provenance when OCR spans are available.

**Stable line identity (Kakebo snap & split, KAK-610):** every extracted line item needs a stable `line_id`, derived deterministically from the page, the reading order and a hash of the line text. Kakebo uses it so participants can be assigned to individual receipt lines, and the assignment must survive re-extraction with a better engine.

*Unblocks Kakebo KAK-205 / KAK-610.*

### DOC-1207: Deterministic line-item parser for Mexican thermal receipts

Many receipts (OXXO, Walmart, Soriana, Costco MX, Starbucks) follow regular layouts.

**Request:** a heuristic parser that works on the OCR layout text. It should extract line items (qty, description, unit price, amount), subtotal, IVA/IEPS, total, folio and the date, with a confidence score, *before* falling back to the LLM. That way the AI-off profile still extracts receipts.

**Acceptance criteria:** at least 90% field accuracy on fixtures for 5 chains.

*Unblocks Kakebo KAK-406.*

### DOC-1208: Replace relative `../../../embedag` path dependency with optional git/workspace dependency

`docula-rag` cannot build unless embedag sits next to it on disk, which breaks downstream consumers like Kakebo.

**Request:** make embedag an optional git dependency behind an `embedag` feature.

**Acceptance criteria:** `cargo build -p docula --no-default-features --features domains,ingest` works in a clean checkout.

*Unblocks Kakebo KAK-403.*

### DOC-1209: Mobile/wasm-friendly feature profile

Kakebo ships on iOS, Android, desktop and web.

**Request:** a `lite` profile containing domains, vision-codes, ingest and the HTTP LLM, with no `tokio/full`, oximedia, candle, oxirag or memmap2.

**Acceptance criteria:**
- CI builds `aarch64-apple-ios`, `aarch64-linux-android` and `wasm32-unknown-unknown` (domains + ingest).
- The binary size is documented.

*Unblocks Kakebo KAK-403.*

### DOC-1210: Reconcile DEV_STATUS and specs with implementation (OCR, LLM backends) + deduplicate Plane milestones

DEV_STATUS marks milestones 1–10 as verified, but several parts are still stubs:
- `DbNetDetector` and `CrnnRecognizer` are heuristic or canned (spec 05 names `rusto-rs` / `ocrs`).
- `OxiLlamaBackend::generate_constrained` ignores its prompts.
- The features `remote-ollama`, `remote-openai` and `local-onnx` are empty.

The Plane board also lists Milestones 9, 10 and 11 twice, with and without the crate suffix.

**Request:**
- Annotate the affected tickets as *scaffolded* rather than *verified*.
- Link them to DOC-1201 and DOC-1202.
- Merge the duplicate modules, so downstream planning can rely on the board.

*Unblocks Kakebo KAK-007.*

## 5. Cross-Cutting Constraints

- **Toolchain:** pure Rust and zero C FFI on the default path. Native runtimes go behind opt-in features only.
- **Async:** no `tokio/full` on library crates. Use a minimal runtime, and keep HTTP clients on rustls.
- **Builds:** default features stay lean enough to build for `aarch64-apple-ios`, `aarch64-linux-android` and `wasm32-unknown-unknown` (where applicable).
- **Determinism:** every model-backed output records the engine, model id and version, and a confidence score, so consumers can store provenance.
