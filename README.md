# invorc: Sovereign Rust Document Intelligence & Structured OCR

`invorc` is a pure-Rust, zero-FFI document intelligence engine designed to ingest PDFs, images, or sets of images, and extract typed, structured data into arbitrary user-defined Rust data structures.

## Core Two-Step Pipeline

1. **Step 1: Content Reading & Spatial Extraction**
   - Ingestion & Routing (`pdf_oxide`, `lopdf`)
   - Pure-Rust Rasterization (`pdf_oxide` with `tiny-skia`)
   - Preprocessing & Deskew (`imageproc`, `oximedia`, `fast_image_resize`)
   - Multi-Image Stitching & Deduplication (`imageproc`, `strsim`)
   - Multi-Lingual Neural OCR (`rusto-rs` / PP-OCRv6 across 50 languages)
2. **Step 2: Structured Data Extraction**
   - Dynamic JSON Schema generation (`schemars`)
   - Local/embedded LLM backends (`oxibonsai`, `candle`, `tract`)
   - Constrained decoding with DFA logits masking (`typedlm`)
   - Strongly typed Serde deserialization & validation

## Documentation Suite

- **[Master Architecture Specification](file:///Users/yasuomaidana/RustRoverProjects/invorc/docs/00_MASTER_SPECIFICATION.md)**
- **[01. Ingestion & Intelligent Routing](file:///Users/yasuomaidana/RustRoverProjects/invorc/docs/01_INGESTION_AND_ROUTING.md)**
- **[02. Rasterization & Preprocessing](file:///Users/yasuomaidana/RustRoverProjects/invorc/docs/02_RASTERIZATION_AND_PREPROCESSING.md)**
- **[03. OCR & Spatial Extraction](file:///Users/yasuomaidana/RustRoverProjects/invorc/docs/03_OCR_AND_SPATIAL_EXTRACTION.md)**
- **[04. LLM & Local/Embedded Backends](file:///Users/yasuomaidana/RustRoverProjects/invorc/docs/04_LLM_AND_LOCAL_EMBEDDED_BACKENDS.md)**
- **[05. Structured Extraction & Constrained Decoding](file:///Users/yasuomaidana/RustRoverProjects/invorc/docs/05_STRUCTURED_EXTRACTION_AND_CONSTRAINED_DECODING.md)**
- **[06. End-to-End Invoice & Generic Schema Examples](file:///Users/yasuomaidana/RustRoverProjects/invorc/docs/06_INVOICE_AND_GENERIC_SCHEMA_EXAMPLE.md)**
- **[07. Development Plan & Milestones](file:///Users/yasuomaidana/RustRoverProjects/invorc/docs/07_DEVELOPMENT_PLAN.md)**
- **[08. Benchmark & Crate Comparison](file:///Users/yasuomaidana/RustRoverProjects/invorc/docs/08_BENCHMARK_AND_COMPARISON.md)**
- **[09. Image Stitching & Overlap Deduplication](file:///Users/yasuomaidana/RustRoverProjects/invorc/docs/09_IMAGE_STITCHING_AND_DEDUPLICATION.md)**
- **[10. RAG & Large Document Retrieval](file:///Users/yasuomaidana/RustRoverProjects/invorc/docs/10_RAG_AND_LARGE_DOCUMENT_RETRIEVAL.md)**
