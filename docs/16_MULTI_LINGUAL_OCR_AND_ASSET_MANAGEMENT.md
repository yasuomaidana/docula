# Module 16: Multi-Lingual OCR & Language Asset Management

## 1. Overview & Architectural Challenges

`docula` is designed to process heterogeneous international documents (invoices, bills, customs forms, shipping manifests) across **50+ languages simultaneously**.

### The Engineering Challenges
1. **Model Weight Bloat**: Shipping neural network weights and full dictionaries for 50+ scripts incurs **~80 MB to 150 MB** of binary footprint.
2. **Memory Footprint**: Embedding all dictionaries directly in RAM causes high memory spikes (RSS > 500 MB) even when processing standard Latin-only invoices.
3. **Mixed-Script Invoices**: Real-world documents frequently mix scripts on a single page (e.g. an English invoice header with a Japanese vendor address and an Arabic bank name).
4. **Zero-FFI Pure Rust Constraint**: Inference must execute without linking to dynamic C libraries (no Tesseract, Poppler, or OpenCV).

---

## 2. Multi-Lingual Architecture & Pipeline

```mermaid
flowchart TD
    DocImg[Document Image / Rendered PDF Pixmap] --> Detect[1. DBNet Text Detection (Language Agnostic)]
    Detect --> Boxes[Polygonal Bounding Contours & Vatti Unclip]
    
    Boxes --> ScriptCheck{"Language Hint or Digital Unicode Range Available?"}
    
    ScriptCheck -- "Latin-Only Hint" --> FastLatin[2a. Lightweight Latin CRNN Model (~10 MB, ocrs/RTen)]
    ScriptCheck -- "Multi-Lingual / Unknown" --> UnifiedCRNN[2b. PP-OCRv6 Multi-Script CRNN (~80 MB, rusto-rs)]
    
    subgraph AssetManager["Language Asset Manager (mmap & Lazy Loading)"]
        Mmap["memmap2 Zero-Copy File Mapping"]
        SharedWeights["Shared Read-Only Page Cache across Threads"]
        Trie["O(1) CTC Character Tokenizer & Dictionary Trie"]
    end
    
    UnifiedCRNN -.-> AssetManager
    FastLatin -.-> AssetManager
    
    FastLatin --> BatchedInference[3. Rayon CPU Batch Inference Across Text Crops]
    UnifiedCRNN --> BatchedInference
    
    BatchedInference --> SpatialReconstruct[4. 2D Spatial Layout & UTF-8 Coordinate Serialization]
```

---

## 3. Cargo Feature Flags Matrix

To eliminate unnecessary bloat for consumers who do not require global scripts, language support is strictly feature-gated:

```toml
[features]
# Default to lightweight Latin OCR for fast compilation and small binary footprint
default = ["ocr-latin"]

# Lightweight pure-Rust Latin OCR via RTen (~10 MB model, low memory)
ocr-latin = ["dep:ocrs"]

# Comprehensive 50-language PP-OCRv6 engine with unified CTC dictionary
ocr-multilingual = ["dep:rusto-rs"]

# Modular language packs enabling fine-grained model asset activation
lang-cjk        = ["ocr-multilingual"] # Chinese (Simplified & Traditional), Japanese, Korean
lang-arabic     = ["ocr-multilingual"] # Arabic, Persian, Urdu
lang-cyrillic   = ["ocr-multilingual"] # Russian, Ukrainian, Bulgarian, Serbian
lang-devanagari = ["ocr-multilingual"] # Hindi, Marathi, Nepali
```

---

## 4. Efficient Asset Management Strategies

### 4.1 Memory Mapping (`memmap2`) & Zero-Copy Inference
Model weights (`det.mnn`, `rec.mnn`) and unified dictionary files (`ppocrv6_dict.txt`) are loaded using memory-mapped files:
- **Zero-Copy**: The operating system maps disk pages directly into virtual memory.
- **Shared Memory**: Multiple worker threads and processes share the same read-only physical memory pages.
- **Low RSS Overhead**: Inactive pages are paged out under memory pressure, keeping the resident set size minimal.

### 4.2 Unicode Block Pre-Filtering for Born-Digital Documents
When documents are born-digital PDFs, `docula-ingest` inspects Unicode character ranges before triggering neural recognition:
- `U+0020..=0x024F`: Basic Latin & Latin Extended $\to$ Route to lightweight Latin pipeline.
- `U+4E00..=0x9FFF`: CJK Unified Ideographs $\to$ Route to CJK multi-lingual pipeline.
- `U+0600..=0x06FF`: Arabic $\to$ Route to Arabic multi-lingual pipeline.

### 4.3 Rayon Thread Parallelism
Text bounding boxes detected by DBNet are grouped by aspect ratio and dispatched concurrently across CPU cores using `rayon::prelude::*`, achieving near-linear multi-core scaling.

---

## 5. Consumer Configuration & Custom Dictionaries

Downstream consumers can specify custom model directories, override dictionary files, or supply pre-downloaded weights:

```rust
use docula::DoculaConfig;

let config = DoculaConfig::builder()
    .with_ocr_model_dir("/opt/models/ppocrv6")
    .with_ocr_languages(vec!["en".into(), "ja".into(), "de".into()])
    .build();
```

---

## 6. Acceptance Criteria & Test Scenarios

### Acceptance Criteria
1. When compiled with `default = ["ocr-latin"]`, binary size does not include Chinese or Arabic model weights.
2. When compiled with `features = ["ocr-multilingual"]`, recognition recognizes English, Japanese, and Arabic on a single document page without requiring manual language switching.
3. Loading models via `memmap2` keeps process RSS strictly below 150 MB during idle state.
4. Consumer model directory overrides locate and load `.mnn` / `.safetensors` model files from the specified path.

### Test Scenarios
- `test_latin_ocr_accuracy`: Assert character error rate (CER) $< 1.0\%$ on standard English invoice text.
- `test_multilingual_mixed_page`: Recognize a test image containing mixed English, Japanese, and Cyrillic text; verify all scripts are accurately transcribed in the UTF-8 spatial map.
- `test_mmap_asset_lazy_loading`: Verify memory allocation remains flat until the first OCR execution call.
- `test_custom_model_dir_fallback`: Configure a non-existent model directory; assert clear, typed error `OcrError::ModelNotFound`.
