//! Example: Application-Specific Promotion Gate
//!
//! Demonstrates how an application-layer consumer defines a custom `PromotionGate`
//! implementing `PromotionPolicy` with business-specific recurrence thresholds (>= 3).

use docula_exemplar::TemplatePair;
use docula_hitl::{CorrectionTriad, HitlError, PromotionPolicy};

/// Application-specific quality gate enforcing minimum recurrence before promotion.
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
}

impl PromotionPolicy for PromotionGate {
    fn evaluate_promotion(
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

    fn promote(
        &self,
        triad: &CorrectionTriad,
        document_type: &str,
    ) -> Result<TemplatePair, HitlError> {
        let metadata = docula_exemplar::ExemplarMetadata {
            exemplar_id: format!("promoted_{}", triad.triad_id),
            vendor_name: triad.metadata.vendor_id.clone(),
            vendor_tax_id: triad.metadata.vendor_id.clone(),
            document_type: document_type.to_string(),
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

fn main() {
    let gate = PromotionGate::new(3);
    println!(
        "Application-specific PromotionGate initialized with min_recurrence={}",
        gate.min_recurrence_count
    );
}
