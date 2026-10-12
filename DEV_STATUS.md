# Dev Status

**Current Focus:** `DOC-202` (Image Filter Chain: Deskew & Binarize)

### Milestone 1: Core Foundation & Config
- [x] DOC-101 Spatial Geometry & DocumentSource (`docula-core`)
- [x] DOC-102 DoculaConfig & Layered Loader (`docula-core`)
- [x] DOC-103 Zero-Storage RAII NonStored<T> (`docula-core`)

### Milestone 2: Preprocess & Ingestion
- [x] DOC-201 Tiny-Skia Pure-Rust Rasterizer (`docula-preprocess`)
- [ ] DOC-202 Image Filter Chain: Deskew & Binarize (`docula-preprocess`)
- [ ] DOC-203 PDF Inspector & Routing Heuristics (`docula-ingest`)

### Milestone 3: OCR, Spatial & Compliance
- [ ] DOC-301 PP-OCRv6 DBNet & CRNN Multi-Lingual (`docula-ocr`)
- [ ] DOC-302 Spatial Reading-Order Clustering (`docula-ocr`)
- [ ] DOC-303 Compliance Action Tags & Redactor (`docula-compliance`)
- [ ] DOC-304 Multi-Lingual Mmap Asset Manager (`docula-ocr`)

### Milestone 4: Schema, DFA Masking & LLM
- [ ] DOC-401 JSON Schema to DFA Compiler (`docula-schema`)
- [ ] DOC-402 Autoregressive Logits Masking (`docula-schema`)
- [ ] DOC-403 Sovereign LlmBackend Trait & Adapters (`docula-llm`)

### Milestone 5: Retrieval & Exemplars
- [ ] DOC-501 Layout-Aware RAG Chunker & Hybrid Index (`docula-rag`)
- [ ] DOC-502 Dual-Stage Exemplar Retrieval (`docula-exemplar`)

### Milestone 6: Active Learning & Feedback
- [ ] DOC-601 Triage Gates & CorrectionTriads (`docula-hitl`)\n- [ ] DOC-602 Tiered Storage & Promotion Engine (`docula-hitl`)

### Milestone 7: Advanced Vision (Codes & Cropping)
- [ ] DOC-701 Pure-Rust Optical Code Mapper (`docula-vision-codes`)
- [ ] DOC-702 Document Screenshots & Region Cropper (`docula-preprocess`)

### Milestone 8: Developer Surface, CLI & Benchmarks
- [ ] DOC-801 High-Level DoculaPipeline & Self-Repair (`docula`)
- [ ] DOC-802 Terminal CLI Utility (`docula-cli`)
- [ ] DOC-803 cargo-geiger Zero-FFI Safety & Criterion Benchmarks (`docula`)
