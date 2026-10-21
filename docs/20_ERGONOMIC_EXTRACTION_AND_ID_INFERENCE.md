# Module 20: Pipeline Ergonomics & Document Identifier Inference

## 1. Overview & Architectural Motivation

In sovereign document intelligence pipelines, consumers interact with the extraction engine under two primary operational models:

1. **Engine-Orchestrated Workflow (External ID Available)**:
   - The document is ingested from an asynchronous message bus (Kafka, AWS SQS, RabbitMQ), a relational database, or a workflow orchestrator (Airflow, Temporal).
   - An authoritative tracking identifier (e.g. `order-98721`, `msg-uuid-v7`, `blob-key-4401`) already exists before the pipeline starts.
   - The consumer requires that this upstream identifier be preserved end-to-end and returned in the resulting [`ExtractionResult`](file:///Users/yasuomaidana/RustRoverProjects/docula/src/pipeline.rs#L113) for auditing and queue routing.

2. **Developer-Centric / Pure Schema Parsing (Zero Prior Knowledge)**:
   - A developer has a file on disk or an in-memory byte buffer, defines a strongly typed Rust domain struct (e.g. `Invoice<SignaturePayload>`, `Receipt`, `MedicalClaim`), and only wants to parse the document into that structure.
   - The developer does not possess an external business identifier and should not be forced to invent or hardcode dummy identifiers like `"doc-1"` or `"cli-doc"`.

To support both personas without compromising API cleanliness or type safety, `Docula` provides dual entrypoints backed by deterministic identifier inference.

---

## 2. Dual Ingestion Patterns

### Pattern 1: Explicit Tracking Identifier (`process_document`)
Use [`DoculaPipeline::process_document`](file:///Users/yasuomaidana/RustRoverProjects/docula/src/pipeline.rs#L236) when the identifier is supplied by an external system:

```rust
use docula::{DoculaPipeline, DocumentSource};
use my_domain::Invoice;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pipeline = DoculaPipeline::new();
    let source = DocumentSource::from_pdf_path("inbound/INV_2026_US_09.pdf");

    // Explicit tracking identifier from upstream queue/engine
    let result = pipeline
        .process_document::<Invoice>("sqs-order-evt-99014", &source)
        .await?;

    assert_eq!(result.document_id, "sqs-order-evt-99014");
    if let Some(invoice) = result.data {
        println!("Processed invoice for {}: total = ${:.2}", invoice.merchant.name, invoice.total);
    }
    Ok(())
}
```

### Pattern 2: Ergonomic Schema Parsing (`process`)
Use [`DoculaPipeline::process`](file:///Users/yasuomaidana/RustRoverProjects/docula/src/pipeline.rs#L201) when you only have the document source and target struct:

```rust
use docula::{DoculaPipeline, DocumentSource};
use my_domain::Invoice;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pipeline = DoculaPipeline::new();
    let source = DocumentSource::from_image_path("scans/invoice_example.png");

    // Automatically infers document_id as "invoice_example" from the file stem
    let result = pipeline
        .process::<Invoice>(&source)
        .await?;

    assert_eq!(result.document_id, "invoice_example");
    let invoice = result.data.expect("Valid parsed invoice");
    println!("Extracted: {:?}", invoice);
    Ok(())
}
```

---

## 3. Deterministic Inference Strategy ([`DocumentSource::infer_document_id`](file:///Users/yasuomaidana/RustRoverProjects/docula/crates/docula-core/src/source.rs#L190))

The inference strategy is encapsulated within [`docula_core::DocumentSource`](file:///Users/yasuomaidana/RustRoverProjects/docula/crates/docula-core/src/source.rs#L6) and guarantees stability, idempotency, and zero panics:

| Source Variant | Inference Rule | Example Input | Resulting `document_id` |
| :--- | :--- | :--- | :--- |
| `PdfPath(path)` | File stem extraction | `/tmp/invoices/INV-2026-001.pdf` | `"INV-2026-001"` |
| `ImagePath(path)` | File stem extraction | `assets/receipt_dec_12.png` | `"receipt_dec_12"` |
| `OfficePath { path, .. }` | File stem extraction | `docs/q3_earnings.xlsx` | `"q3_earnings"` |
| `PdfBytes(bytes)` | Deterministic SHA-256 (first 16 hex chars) | In-memory PDF buffer | `"doc-a3f8901b2c45e890"` |
| `ImageBytes { data, .. }` | Deterministic SHA-256 (first 16 hex chars) | In-memory image buffer | `"doc-7e10b42f6190da22"` |
| `OfficeBytes { data, .. }` | Deterministic SHA-256 (first 16 hex chars) | In-memory spreadsheet buffer | `"doc-c90a12e8419bd3fa"` |
| `BatchFiles(paths)` | First file stem + batch count | `["page1.png", "page2.png"]` | `"page1-batch-2"` |
| `BatchImages(images)` | Combined SHA-256 over all images | Vector of byte buffers | `"doc-batch-55a01bc8329410fe"` |

### Key Guarantees:
- **Idempotency**: Re-processing identical in-memory byte payloads produces the exact same document ID across runs and platforms.
- **Zero-FFI & Pure-Rust**: Powered by `sha2`, avoiding platform-dependent hashing like `std::hash::DefaultHasher`.
- **Safe Fallbacks**: If a filesystem path has an empty stem (e.g. root `/`), it safely falls back to `"doc-unnamed-file"` without panic.

---

## 4. Architectural Complexity & Scrum Alignment Audit

### Pattern Complexity Audit (`/pattern-complexity-auditor`)
- **Cyclomatic Complexity**: `infer_document_id` maintains CC $\le 6$ and cognitive complexity $\le 4$, utilizing pattern matching over enum variants and clean helper decomposition (`digest_to_id`).
- **No Hidden Panics**: Zero occurrences of `.unwrap()` or `.expect()` in library code; all path extractions safely chain `.and_then()` and `.filter()`.
- **Zero-Cost Delegation**: `DoculaPipeline::process` delegates to `process_document` in 3 lines without duplicating execution branches.

### Scrum Master Alignment (`/scrum-master-alignment`)
- **Definition of Done (DoD)**:
  - Both use cases are verified by automated integration tests ([`tests/ergonomic_pipeline_tests.rs`](file:///Users/yasuomaidana/RustRoverProjects/docula/tests/ergonomic_pipeline_tests.rs)).
  - Unit tests verify deterministic hash consistency, path stem isolation, and batch handling in [`crates/docula-core/src/source.rs`](file:///Users/yasuomaidana/RustRoverProjects/docula/crates/docula-core/src/source.rs#L270).
  - CLI consumer in [`crates/docula-cli/src/main.rs`](file:///Users/yasuomaidana/RustRoverProjects/docula/crates/docula-cli/src/main.rs#L113) now leverages `pipeline.process::<GenericDocumentOutput>`, eliminating hardcoded `"cli-doc"` placeholders.
