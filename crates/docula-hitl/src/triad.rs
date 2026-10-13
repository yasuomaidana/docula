use chrono::{DateTime, Utc};
use docula_ocr::SpatialLayoutMap;
use docula_schema::ValidationError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// RFC 6902-style JSON Patch difference between model output and corrected ground truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonPatchDiff {
    /// Ordered list of diff operations.
    pub patch: Vec<serde_json::Value>,
}

impl JsonPatchDiff {
    /// Computes a top-level key diff between model output and ground truth.
    #[must_use]
    pub fn compute(bad: &serde_json::Value, corrected: &serde_json::Value) -> Self {
        let mut patch = Vec::new();

        if let (Some(b_obj), Some(c_obj)) = (bad.as_object(), corrected.as_object()) {
            for (k, v) in c_obj {
                if b_obj.get(k) != Some(v) {
                    patch.push(serde_json::json!({
                        "op": "replace",
                        "path": format!("/{k}"),
                        "value": v,
                        "old_value": b_obj.get(k)
                    }));
                }
            }
        }

        Self { patch }
    }
}

/// Metadata describing the human review context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriadMetadata {
    /// Optional vendor or merchant identifier.
    pub vendor_id: Option<String>,
    /// 64-bit SimHash geometric layout fingerprint.
    pub layout_fingerprint: u64,
    /// Identifier of the reviewing human specialist.
    pub user_id: Option<String>,
    /// Review and correction duration in milliseconds.
    pub review_latency_ms: u64,
    /// UTC timestamp of correction capture.
    pub captured_at: DateTime<Utc>,
}

/// Immutable record capturing a human-reviewed extraction correction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CorrectionTriad {
    /// Unique identifier of this correction triad (UUIDv7).
    pub triad_id: Uuid,
    /// SHA-256 hash of the input document artifact.
    pub source_artifact_sha256: String,
    /// Intermediate OCR spatial layout with `@censor` regions scrubbed.
    pub intermediate_ocr_layout: SpatialLayoutMap,
    /// Erroneous model output generated prior to human review.
    pub model_output_bad: serde_json::Value,
    /// Specific gate validation errors that triggered review.
    pub gate_errors: Vec<ValidationError>,
    /// Human-verified ground truth payload.
    pub ground_truth_corrected: serde_json::Value,
    /// Diff representing the corrections made.
    pub diff: JsonPatchDiff,
    /// Contextual review metadata.
    pub metadata: TriadMetadata,
}

impl CorrectionTriad {
    /// Computes cryptographic SHA-256 hex digest of raw input bytes.
    #[must_use]
    pub fn hash_artifact(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = hasher.finalize();
        digest.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Computes cryptographic SHA-256 hex digest of this entire triad record.
    #[must_use]
    pub fn compute_record_hash(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap_or_default();
        Self::hash_artifact(serialized.as_bytes())
    }
}
