//! Pluggable active learning promotion policy abstraction for docula.
//!
//! Decoupled from application-specific vendor or recurrence heuristics,
//! empowering developers to define their own custom promotion quality gates.

use crate::error::HitlError;
use crate::triad::CorrectionTriad;
use docula_exemplar::{ExemplarMetadata, TemplatePair};

/// Pluggable quality gate policy determining when human correction triads
/// qualify for promotion from cold storage into the active few-shot exemplar pool.
pub trait PromotionPolicy: Send + Sync {
    /// Evaluates whether a captured `CorrectionTriad` satisfies promotion criteria.
    fn evaluate_promotion(
        &self,
        triad: &CorrectionTriad,
        recurrence_count: usize,
        existing_pool: &[TemplatePair],
    ) -> bool;

    /// Transforms a validated `CorrectionTriad` into a curated `TemplatePair`.
    fn promote(
        &self,
        triad: &CorrectionTriad,
        document_type: &str,
    ) -> Result<TemplatePair, HitlError>;
}

/// Generic threshold-based promotion policy allowing developers to configure
/// minimum recurrence counts and deduplication.
#[derive(Clone, Debug)]
pub struct ThresholdPromotionPolicy {
    /// Minimum times a layout fingerprint must recur before promotion.
    pub min_recurrence_count: usize,
}

impl ThresholdPromotionPolicy {
    /// Creates a new `ThresholdPromotionPolicy` with the given minimum recurrence threshold.
    #[must_use]
    pub const fn new(min_recurrence_count: usize) -> Self {
        Self {
            min_recurrence_count,
        }
    }
}

impl Default for ThresholdPromotionPolicy {
    fn default() -> Self {
        Self {
            min_recurrence_count: 1,
        }
    }
}

impl PromotionPolicy for ThresholdPromotionPolicy {
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
        let metadata = ExemplarMetadata {
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
