//! Example: Application-Specific Promotion Gate
//!
//! Demonstrates how an application-layer consumer defines a custom `PromotionGate`
//! implementing `PromotionPolicy` with business-specific recurrence thresholds (>= 3).

use docula_exemplar::TemplatePair;
use docula_hitl::{CorrectionTriad, PromotionPolicy, ThresholdPromotionPolicy};

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
        // Business rule: the recurrence threshold, then the stock de-duplication by layout.
        ThresholdPromotionPolicy::new(self.min_recurrence_count).evaluate_promotion(
            triad,
            recurrence_count,
            existing_pool,
        )
    }
}

fn main() {
    let gate = PromotionGate::new(3);
    println!(
        "Application-specific PromotionGate initialized with min_recurrence={}",
        gate.min_recurrence_count
    );
}
