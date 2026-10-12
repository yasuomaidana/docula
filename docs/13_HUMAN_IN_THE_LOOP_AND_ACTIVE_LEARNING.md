# Module 12: Human-in-the-Loop (HITL) & Active Learning Flywheel

## 1. Overview & Purpose

In production Document AI deployments, edge-case layouts, faded print, or ambiguous OCR readings inevitably lead to occasional extraction errors. Unsupervised feeding of corrections directly into an active LLM prompt causes **prompt poisoning**, **prompt bloat**, and **latency degradation**.

The **Human-in-the-Loop (HITL) & Active Learning Flywheel** module establishes a disciplined, four-stage feedback loop that intercepts failures, captures human corrections as immutable **Correction Triads**, stores them in cold audit storage, and promotes high-signal exemplars to the active few-shot pool through strict verification gates.

```mermaid
flowchart TD
    subgraph Stage1["Stage 1: Automated Triage Gates"]
        RawLLM[LLM Output JSON] --> SerdeGate{"Serde Schema Invariants"}
        SerdeGate -->|Invalid| TriageQueue["Triage Review Queue"]
        SerdeGate -->|Valid| MathGate{"DocuValidate: Domain Rules (e.g. Subtotal + Tax == Total)"}
        MathGate -->|Failed| TriageQueue
        MathGate -->|Valid| ConfGate{"Logits Token Confidence >= Threshold"}
        ConfGate -->|Low Confidence| TriageQueue
        ConfGate -->|Passed| ProductionOut["Production Consumption (Downstream)"]
    end

    subgraph Stage2["Stage 2: Human Review & Triad Capture"]
        TriageQueue --> HumanReviewer["Human Specialist Reviews & Edits JSON"]
        HumanReviewer --> Sanitize["Apply Compliance Directives (Redact Sensitive BBoxes)"]
        Sanitize --> TriadAssemble["Assemble Immutable Correction Triad"]
    end

    subgraph Stage3["Stage 3: Two-Tier Storage Architecture"]
        TriadAssemble --> ColdArchive["Cold Archive (/data/corrections/ - Append-Only JSONL)"]
        ColdArchive -.-> RegressionTests["Offline Regression Benchmarks & Local LoRA Fine-Tuning"]
    end

    subgraph Stage4["Stage 4: Curation & Exemplar Promotion"]
        ColdArchive --> PromotionGate{"Promotion Evaluator:<br/>1. Recurrence count >= 3<br/>2. Passes Serde & DocuValidate<br/>3. Non-conflicting layout hash"}
        PromotionGate -->|Promoted| CuratedPool["Curated Few-Shot Pool (/data/examples/)"]
        CuratedPool --> DynamicRetrieval["Module 11: Dynamic Dual-Stage Retrieval"]
    end
```

---

## 2. Technical Stack & Pure-Rust Dependencies

| Crate           | Version Target | Purpose                                                                 |
|:----------------|:---------------|:------------------------------------------------------------------------|
| `docula-core`   | Workspace      | `SpatialLayoutMap`, `ValidationError`, and domain validation traits.    |
| `docula-exemplar`| Workspace     | Exemplar metadata schemas and layout fingerprint integration.           |
| `serde` & `serde_json` | `^1.0`  | Serialization of correction triads and RFC 6902 JSON patch diffs.       |
| `uuid`          | `^1.10`        | Unique identification of correction records (`Uuid::now_v7()`).         |
| `chrono`        | `^0.4`         | UTC timestamps for audit trails and time-to-live policies.              |
| `sha2`          | `^0.10`        | Cryptographic SHA-256 hashing of source documents.                      |
| `testcontainers`| `^0.28` (dev)  | Mandatory persistent layer integration testing tool ([rust.testcontainers.org](https://rust.testcontainers.org)). |

---

## 3. Four-Stage Active Learning Flywheel

### 3.1 Stage 1: Automated Triage Gates
Before presenting extracted data to downstream consumers, all outputs pass through deterministic verification:
1. **Schema Invariant Enforcement**: Ensures types, required fields, and non-empty arrays match the Rust struct (`serde_json::from_str::<T>`).
2. **Domain Integrity Validation (`DocuValidate`)**: Verifies arithmetic equations (e.g. $\sum \text{line\_items} == \text{subtotal}$, $\text{subtotal} + \text{tax} == \text{total}$ within $\pm \$0.05$ rounding tolerance) and cross-field constraints.
3. **Logits Token Confidence**: If the lowest token probability or average sequence confidence drops below the threshold (default: $p < 0.85$), the document is routed to human triage.

### 3.2 Stage 2: Correction Triad Capture
When a reviewer corrects an erroneous extraction, the correction is packaged as an immutable **Correction Triad**:
- **Source Artifact**: The cryptographic SHA-256 hash of the original input PDF/image (or encrypted buffer if within TTL).
- **Intermediate Input**: The raw OCR `SpatialLayoutMap` (with any `@censor` regions permanently scrubbed).
- **Model Output (Bad)**: The original erroneous JSON output alongside specific gate error messages.
- **Ground Truth (Corrected)**: The final, human-validated JSON payload confirmed by the reviewer.
- **Metadata & Diff**: RFC 6902 JSON Patch diff, vendor ID, 64-bit layout fingerprint, user ID, and review latency.

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use docula_core::{DoculaError, SpatialLayoutMap, ValidationError};

/// RFC 6902 JSON Patch difference between bad output and corrected ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonPatchDiff {
    pub patch: Vec<serde_json::Value>,
}

/// Metadata describing the context of human triage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriadMetadata {
    pub vendor_id: Option<String>,
    pub layout_fingerprint: u64,
    pub user_id: Option<String>,
    pub review_latency_ms: u64,
    pub captured_at: DateTime<Utc>,
}

/// Immutable record capturing a human correction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionTriad {
    pub triad_id: Uuid,
    pub source_artifact_sha256: String,
    pub intermediate_ocr_layout: SpatialLayoutMap,
    pub model_output_bad: serde_json::Value,
    pub gate_errors: Vec<ValidationError>,
    pub ground_truth_corrected: serde_json::Value,
    pub diff: JsonPatchDiff,
    pub metadata: TriadMetadata,
}
```

### 3.3 Stage 3: Two-Tier Storage Architecture
Unchecked writing of corrections into the active few-shot prompt causes severe quality regressions. The storage architecture is strictly partitioned into two tiers:

```
/data/
├── corrections/                 # TIER 1: Cold Storage (Append-only JSONL / Parquet / DB)
│   ├── 2026-10-10_triads.jsonl  # Immutable regression test suite and fine-tuning corpus
│   └── 2026-10-11_triads.jsonl
└── examples/                    # TIER 2: Curated Active Pool (Top-Signal Exemplars)
    ├── index.json               # HNSW and layout fingerprint lookup index
    ├── pge_electric_v1.json     # Promoted template pairings
    └── verizon_telecom_v2.json
```

1. **Cold Archive (`/data/corrections/`)**:
   - Append-only immutable log.
   - Functions as an automated regression benchmark test suite (`cargo test --test hitl_regressions`).
   - Serves as the training dataset for future local LoRA adapter fine-tuning.
   - **Persistent Storage Mandate**: Storage adapters (filesystem, object store, database backends) MUST have integration tests written using [Testcontainers for Rust](https://rust.testcontainers.org).
2. **Curated Few-Shot Pool (`/data/examples/`)**:
   - High-value, deduplicated exemplars accessible to Module 11's prompt builder.
   - Strictly curated to prevent prompt bloating.

### 3.4 Stage 4: Promotion Gate & Evaluation Pipeline
A correction triad in `/data/corrections/` is never automatically injected into prompt context. It must be promoted to `/data/examples/` by satisfying three conditions:
1. **Recurrence Threshold**: The same vendor or layout fingerprint has triggered at least $N$ corrections (default: $N \ge 3$), proving it is a recurring pattern rather than an isolated anomaly.
2. **Full Validation Pass**: The corrected JSON deserializes cleanly and passes all `DocuValidate` rules with zero errors.
3. **Deduplication Check**: The curated pool does not already contain an exemplar with the identical layout fingerprint.

```rust
pub trait TriadPromotionEvaluator {
    /// Evaluates whether a triad qualifies for promotion to /data/examples/
    fn evaluate_promotion(
        &self,
        triad: &CorrectionTriad,
        existing_pool: &[TemplatePair],
    ) -> Result<bool, DoculaError>;

    /// Executes the promotion, saving the template pair to /data/examples/
    fn promote(
        &self,
        triad: &CorrectionTriad,
    ) -> Result<TemplatePair, DoculaError>;
}
```

---

## 4. Configuration in `Docula.toml`

```toml
[hitl]
enabled = true
cold_archive_directory = "./data/corrections"
curated_pool_directory = "./data/examples"

[hitl.triage_gates]
min_token_confidence = 0.85
enforce_Docula_validate = true
enforce_serde_invariants = true

[hitl.promotion_policy]
min_recurrence_count = 3
auto_promote = false           # If false, requires manual CLI command: docula hitl promote <id>
max_pool_size_per_vendor = 2
```

---

## 5. Testing & Verification

1. **Triad Immutability Test**:
   - `test_triad_serialization_and_hash_integrity()`: Validates that serialization of a `CorrectionTriad` produces valid JSON and matches the SHA-256 fingerprint.
2. **Triage Gating Test**:
   - `test_invoice_math_failure_routes_to_triage()`: Injects an invoice where `line_items` sum does not match `total_amount`, asserting immediate routing to `TriageQueue`.
3. **Promotion Policy Test**:
   - `test_promotion_gate_blocks_single_occurrence()`: Verifies that a one-off correction is stored in cold storage but blocked from active few-shot prompt injection.
4. **Persistent Layer Integration Test (Mandatory via [Testcontainers](https://rust.testcontainers.org))**:
   - `test_append_only_cold_storage_persistence_with_testcontainers()`: Spins up an isolated throwaway container (e.g., MinIO or PostgreSQL via `testcontainers`) to verify cold archive ingestion, atomic appending, cryptographic checksum verification across restarts, and read-after-write consistency under realistic socket and filesystem semantics.
   - All persistent layer integrations are strictly mandated to execute against real containerized instances powered by [Testcontainers for Rust](https://rust.testcontainers.org); unit mocks alone are prohibited for storage verification.
