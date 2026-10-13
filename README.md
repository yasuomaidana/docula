# docula

A sovereign, zero-FFI, pure-Rust multi-modal document intelligence engine for transforming messy documents (invoices, receipts, bills, bank statements) into strongly typed, validated domain models.

## Core Tenets

- **Pure Rust (Zero-FFI)**: No dynamic linkage against C/C++ libraries (`tesseract`, `poppler`, `opencv`, `zbar`, `mupdf`). Compiles everywhere Rust compiles.
- **Two-Step Architecture**:
  1. *Step 1: Ingestion & Spatial Layout Reconstruction*: PDF unpacking, `tiny-skia` pure-Rust rendering, PP-OCRv6 neural text detection/recognition, optical code detection, document screenshots & visual ROI region cropping, and 2D spatial layout reconstruction.
  2. *Step 2: Schema-Driven Structured Extraction*: Local/embedded LLM backends (`oxibonsai`, `candle`, `tract`), constrained decoding with DFA logits masking, strongly typed Serde deserialization & validation.

## Documentation Suite

- **[Master Architecture Specification](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/00_MASTER_SPECIFICATION.md)**
- **[01. Development Plan & Engineering Roadmap](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/01_DEVELOPMENT_PLAN.md)**
- **[02. Design Patterns & Architecture Blueprint](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/02_DESIGN_PATTERNS_AND_ARCHITECTURE.md)**
- **[03. Ingestion & Intelligent Routing](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/03_INGESTION_AND_ROUTING.md)**
- **[04. Rasterization & Preprocessing](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/04_RASTERIZATION_AND_PREPROCESSING.md)**
- **[05. OCR & Spatial Extraction](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/05_OCR_AND_SPATIAL_EXTRACTION.md)**
- **[06. LLM & Local/Embedded Backends](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/06_LLM_AND_LOCAL_EMBEDDED_BACKENDS.md)**
- **[07. Structured Extraction & Constrained Decoding](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/07_STRUCTURED_EXTRACTION_AND_CONSTRAINED_DECODING.md)**
- **[08. End-to-End Invoice & Generic Schema Examples](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/08_INVOICE_AND_GENERIC_SCHEMA_EXAMPLE.md)**
- **[09. Benchmark & Crate Comparison](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/09_BENCHMARK_AND_COMPARISON.md)**
- **[10. Image Stitching & Overlap Deduplication](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/10_IMAGE_STITCHING_AND_DEDUPLICATION.md)**
- **[11. RAG & Large Document Retrieval](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/11_RAG_AND_LARGE_DOCUMENT_RETRIEVAL.md)**
- **[12. Few-Shot & Exemplar Management](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/12_FEW_SHOT_AND_EXEMPLAR_MANAGEMENT.md)**
- **[13. Human-in-the-Loop & Active Learning](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/13_HUMAN_IN_THE_LOOP_AND_ACTIVE_LEARNING.md)**
- **[14. Edge Compliance & Non-Storing Directives](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/14_COMPLIANCE_AND_NON_STORING.md)**
- **[15. Consumer Configuration & TOML Strategy](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/15_CONSUMER_CONFIGURATION_AND_TOML.md)**
- **[16. Multi-Lingual OCR & Language Asset Management](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/16_MULTI_LINGUAL_OCR_AND_ASSET_MANAGEMENT.md)**
- **[17. Pure-Rust Optical Code Extraction](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/17_OPTICAL_CODE_EXTRACTION.md)**
- **[18. Document Screenshots & Region Cropping](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/18_DOCUMENT_SCREENSHOTS_AND_REGION_CROPPING.md)**
- **[19. Ofimatic & Office Suite Ingestion](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/19_OFIMATIC_OFFICE_SUITE_INGESTION.md)**
- **[20. Pipeline Ergonomics & Document Identifier Inference](file:///Users/yasuomaidana/RustRoverProjects/docula/docs/20_ERGONOMIC_EXTRACTION_AND_ID_INFERENCE.md)**
