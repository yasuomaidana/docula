use crate::error::HitlError;
use crate::triad::CorrectionTriad;
use docula_exemplar::{ExemplarMetadata, TemplatePair};

/// Evaluator enforcing quality gates before promoting cold archive records to the active few-shot pool.
#[derive(Clone, Debug)]
pub struct PromotionGate {
    /// Minimum times a layout fingerprint or vendor must recur before promotion (default: 3).
    pub min_recurrence_count: usize,
}

impl Default for PromotionGate {
    fn default() -> Self {
        Self {
            min_recurrence_count: 3,
        }
    }
}

impl PromotionGate {
    /// Creates a new `PromotionGate` with the given minimum recurrence threshold.
    #[must_use]
    pub const fn new(min_recurrence_count: usize) -> Self {
        Self {
            min_recurrence_count,
        }
    }

    /// Evaluates if a candidate triad qualifies for promotion to the curated few-shot pool.
    ///
    /// Conditions:
    /// 1. `recurrence_count >= min_recurrence_count` (verified pattern rather than one-off anomaly).
    /// 2. Deduplication check: pool does not already contain an exemplar with the identical fingerprint.
    #[must_use]
    pub fn evaluate_promotion(
        &self,
        triad: &CorrectionTriad,
        recurrence_count: usize,
        existing_pool: &[TemplatePair],
    ) -> bool {
        if recurrence_count < self.min_recurrence_count {
            return false;
        }

        let is_duplicate = existing_pool
            .iter()
            .any(|t| t.metadata.layout_fingerprint == triad.metadata.layout_fingerprint);

        !is_duplicate
    }

    /// Converts a qualified `CorrectionTriad` into a curated `TemplatePair` for few-shot injection.
    pub fn promote(
        &self,
        triad: &CorrectionTriad,
        document_type: impl Into<String>,
    ) -> Result<TemplatePair, HitlError> {
        let metadata = ExemplarMetadata {
            exemplar_id: format!("promoted_{}", triad.triad_id),
            vendor_name: triad.metadata.vendor_id.clone(),
            vendor_tax_id: triad.metadata.vendor_id.clone(),
            document_type: document_type.into(),
            layout_fingerprint: triad.metadata.layout_fingerprint,
            created_at_unix: triad.metadata.captured_at.timestamp().max(0) as u64,
        };

        Ok(TemplatePair {
            metadata,
            source_artifact_path: None,
            spatial_layout: triad.intermediate_ocr_layout.clone(),
            ground_truth_json: triad.ground_truth_corrected.clone(),
        })
    }
}
