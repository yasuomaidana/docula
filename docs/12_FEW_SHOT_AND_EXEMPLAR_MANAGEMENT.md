# Module 11: Few-Shot Exemplar Management & Dual-Stage Retrieval

## 1. Overview & Purpose

Certain document types—such as utility bills (electricity, water, gas), tax forms, customs declarations, and specialized medical records—possess rigid or idiosyncratic layout architectures. While zero-shot LLM inference is effective for standard invoices, complex layout variations can lead to field hallucination or misattributed line items.

The **Few-Shot Exemplar Management** module enables users to register reference examples (image or PDF + expected JSON structure). During inference, the engine identifies similar incoming documents and dynamically injects the most relevant exemplar into the LLM context.

```mermaid
flowchart TD
    subgraph Registration["1. Exemplar Registration & Ingestion"]
        ExDoc[Example Image / PDF] --> ExOCR[Step 1: Rasterization & OCR]
        ExJSON[Expected JSON Target] --> ExPair[Template Pairing Engine]
        ExOCR --> ExLayout[Ground-Truth SpatialLayoutMap]
        ExLayout --> ExPair
        ExPair --> ExMeta[Generate Layout Fingerprint & Metadata]
        ExMeta --> CuratedStore["Curated Exemplar Pool (/data/examples/)"]
    end

    subgraph QueryPipeline["2. Dual-Stage Exemplar Retrieval Pipeline"]
        TargetDoc[Incoming Document] --> TargetOCR[Step 1: SpatialLayoutMap]
        TargetOCR --> S1{"Stage 1: Fast Deterministic Match"}
        S1 -->|Vendor ID / Tax ID Match| FoundS1["Top Exemplar (<1ms)"]
        S1 -->|Geometric Layout Hash Match| FoundS1
        S1 -->|No Exact Match / Ambiguous| S2["Stage 2: Semantic Layout Vector Match (HNSW)"]
        S2 --> TopK["Top 1–2 Exemplars"]
        FoundS1 --> TopK
        S2 -->|Below Similarity Threshold| ZeroShot["Zero-Shot Fallback (0 Exemplars)"]
    end

    subgraph PromptBuilder["3. Dynamic Few-Shot Prompt Construction"]
        TopK --> PromptAssemble[FewShotPromptBuilder]
        ZeroShot --> PromptAssemble
        TargetOCR --> PromptAssemble
        PromptAssemble --> LLM[LlmBackend Constrained Inference]
    end
```

---

## 2. Technical Stack & Pure-Rust Dependencies

| Crate           | Version Target | Purpose                                                                    |
|:----------------|:---------------|:---------------------------------------------------------------------------|
| `docula-core`   | Workspace      | `SpatialLayoutMap`, `BoundingBox`, and core document primitives.          |
| `docula-ocr`    | Workspace      | Step 1 OCR layout extraction for exemplar image inputs.                   |
| `serde` & `serde_json` | `^1.0`  | Serialization/deserialization of expected ground-truth JSON payloads.      |
| `toml`          | `^0.8`         | Configuration parser for `Docula.toml` workspace settings.                 |
| `hnsw_rs`       | `^0.3`         | Pure-Rust approximate nearest neighbor search for Stage 2 vector matching. |
| `strsim`        | `^0.11`        | Fast string similarity for vendor identification and token matching.       |

---

## 3. Dual-Stage Retrieval Architecture

To prevent prompt bloat, high token costs, and inference latency, the system never injects an entire library of examples into a prompt. Instead, it utilizes a **Dual-Stage Retrieval Strategy** capped at a maximum of 1–2 exemplars:

```
Incoming Document ──► [Stage 1: Fast Deterministic (<1ms)] ──► Match Found? ──► YES ──► Inject Top Exemplar
                                 │
                                 └──► NO / Ambiguous
                                        │
                                        ▼
                             [Stage 2: HNSW Vector Search] ──► Cosine >= 0.82? ──► YES ──► Inject Exemplar
                                        │
                                        └──► NO ──► Zero-Shot Fallback
```

### 3.1 Stage 1: Fast Deterministic Matching (<1 ms)
Stage 1 executes on CPU with zero neural network overhead:
1. **Vendor / Entity Exact Match**: Checks extracted vendor tax IDs, VAT numbers, or merchant strings against exemplar metadata.
2. **64-Bit Geometric Layout Fingerprinting (SimHash)**:
   - Each bounding box $(x_0, y_0, x_1, y_1)$ and its normalized spatial bucket are converted into a spatial feature hash.
   - The document's layout is summarized into a 64-bit integer fingerprint.
   - If the Hamming distance between the incoming document and an exemplar fingerprint is $\le 3$, it represents an identical form layout (e.g., standard regional electricity bill template).

### 3.2 Stage 2: Semantic Layout Vector Matching (HNSW Fallback)
If Stage 1 does not yield an exact match:
1. The spatial text tokens and structural headers are passed through a lightweight pure-Rust layout embedding model (e.g. `bge-small-en` or layout-aware n-gram vectors).
2. An in-memory HNSW index (`hnsw_rs`) queries nearest exemplar candidates.
3. If the highest cosine similarity $\ge \tau_{\text{threshold}}$ (default: `0.82`), the candidate is selected.
4. If no candidate exceeds the threshold, the system automatically falls back to **zero-shot inference**, preserving token economy and avoiding misleading context.

---

## 4. Exemplar Registration & Template Pairing

### 4.1 Data Structures

```rust
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use docula_core::{BoundingBox, InvorcError, SpatialLayoutMap};

/// Metadata identifying a curated document exemplar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExemplarMetadata {
    pub exemplar_id: String,
    pub vendor_name: Option<String>,
    pub vendor_tax_id: Option<String>,
    pub document_type: String, // e.g. "electricity_bill", "utility", "customs_declaration"
    pub layout_fingerprint: u64,
    pub created_at_unix: u64,
}

/// A registered exemplar pairing spatial OCR representation with ground truth JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatePair {
    pub metadata: ExemplarMetadata,
    pub source_artifact_path: Option<PathBuf>,
    pub spatial_layout: SpatialLayoutMap,
    pub ground_truth_json: serde_json::Value,
}
```

### 4.2 Configuration via `Docula.toml`

Default behavior and exemplar registration paths can be configured centrally in `Docula.toml`:

```toml
[exemplars]
enabled = true
pool_directory = "./data/examples"
max_injected_exemplars = 2
similarity_threshold = 0.82

[exemplars.stage1]
enable_vendor_match = true
enable_layout_fingerprint = true
max_fingerprint_hamming_distance = 3

[exemplars.stage2]
enable_hnsw_fallback = true
metric = "cosine"
ef_search = 32

[[exemplars.registered]]
id = "pge_electric_bill_v1"
document_type = "electricity_bill"
vendor_name = "Pacific Gas and Electric"
vendor_tax_id = "US-94-1234567"
file = "./examples/pge_bill_sample.pdf"
ground_truth = "./examples/pge_bill_expected.json"
```

---

## 5. Few-Shot Prompt Construction

The prompt builder wraps the extracted spatial text and pairs it with the selected exemplars. When exemplars are omitted, the prompt seamlessly collapses into zero-shot mode without extra tokens:

```rust
pub struct FewShotPromptBuilder;

impl FewShotPromptBuilder {
    pub fn build(
        system_instructions: &str,
        exemplars: &[TemplatePair],
        target_spatial_map: &SpatialLayoutMap,
    ) -> String {
        let mut prompt = String::with_capacity(4096);
        prompt.push_str(system_instructions);
        prompt.push_str("\n\n");

        if !exemplars.is_empty() {
            prompt.push_str("### REFERENCE DOCUMENT EXAMPLES:\n");
            prompt.push_str("Learn the spatial key-value mappings and layout structures from these verified examples.\n\n");

            for (idx, ex) in exemplars.iter().enumerate() {
                prompt.push_str(&format!("--- [Example {}] Document Type: {} ---\n", idx + 1, ex.metadata.document_type));
                prompt.push_str("Spatial Layout Input:\n");
                prompt.push_str(&ex.spatial_layout.format_spatial_layout());
                prompt.push_str("\nTarget JSON Output:\n");
                prompt.push_str(&serde_json::to_string_pretty(&ex.ground_truth_json).unwrap_or_default());
                prompt.push_str("\n\n");
            }

            prompt.push_str("### TARGET DOCUMENT FOR EXTRACTION:\n");
        }

        prompt.push_str("Extract all data strictly matching the requested schema from this document:\n");
        prompt.push_str(&target_spatial_map.format_spatial_layout());
        prompt.push_str("\nJSON Output:");
        prompt
    }
}
```

---

## 6. Testing & Verification

1. **Deterministic Fingerprint Invariance**:
   - `test_layout_fingerprint_rotation_tolerance()`: Asserts that layout fingerprints remain stable across minor $\pm 1^\circ$ rotations or 5% scaling differences.
2. **Retrieval Speed Benchmark**:
   - `test_stage1_retrieval_latency()`: Asserts that Stage 1 fingerprint queries complete in $< 500\,\mu\text{s}$ across an index of 1,000 templates.
3. **Zero-Shot Token Verification**:
   - `test_zero_shot_prompt_overhead()`: Asserts that when no matching exemplar is found, prompt size is identical to zero-shot baseline.
