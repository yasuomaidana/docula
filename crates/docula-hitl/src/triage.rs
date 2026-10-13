use docula_ocr::SpatialLayoutMap;
use docula_schema::ValidationError;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Decision outcome emitted by automated verification gates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriageDecision {
    /// Extraction meets all schema invariants, arithmetic rules, and confidence thresholds.
    Pass,
    /// Extraction failed one or more gates and must be reviewed by a human specialist.
    NeedsReview {
        /// Explicit gate rejection reasons.
        reasons: Vec<String>,
    },
}

/// Verification gate evaluating token confidence and domain consistency rules.
#[derive(Clone, Debug)]
pub struct TriageGate {
    /// Minimum allowed token confidence score $[0.0, 1.0]$.
    pub min_token_confidence: f32,
    /// Whether domain validation rules are enforced.
    pub enforce_domain_validation: bool,
}

impl Default for TriageGate {
    fn default() -> Self {
        Self {
            min_token_confidence: 0.85,
            enforce_domain_validation: true,
        }
    }
}

impl TriageGate {
    /// Constructs a new `TriageGate` with the given minimum confidence threshold.
    #[must_use]
    pub const fn new(min_token_confidence: f32) -> Self {
        Self {
            min_token_confidence,
            enforce_domain_validation: true,
        }
    }

    /// Evaluates extraction results against confidence and domain consistency gates.
    #[must_use]
    pub fn evaluate(
        &self,
        token_confidence: f32,
        domain_errors: &[ValidationError],
    ) -> TriageDecision {
        let mut reasons = Vec::new();

        if token_confidence < self.min_token_confidence {
            reasons.push(format!(
                "Token confidence ({token_confidence:.2}) below required threshold ({:.2})",
                self.min_token_confidence
            ));
        }

        if self.enforce_domain_validation && !domain_errors.is_empty() {
            for err in domain_errors {
                reasons.push(format!(
                    "Domain validation error at '{}': {}",
                    err.field_path, err.error_message
                ));
            }
        }

        if reasons.is_empty() {
            TriageDecision::Pass
        } else {
            TriageDecision::NeedsReview { reasons }
        }
    }
}

/// An extraction item queued for human specialist review.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TriageItem {
    /// Unique item identifier.
    pub id: Uuid,
    /// Model-generated candidate JSON output.
    pub model_output: serde_json::Value,
    /// Rejection reasons that routed this item to triage.
    pub reasons: Vec<String>,
    /// Intermediate OCR spatial layout.
    pub spatial_layout: SpatialLayoutMap,
    /// Extraction token confidence score.
    pub confidence: f32,
}

/// Thread-safe in-memory review queue holding flagged extractions.
#[derive(Clone, Debug, Default)]
pub struct TriageQueue {
    items: Arc<Mutex<Vec<TriageItem>>>,
}

impl TriageQueue {
    /// Constructs an empty `TriageQueue`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueues an item for human review.
    pub fn enqueue(&self, item: TriageItem) {
        if let Ok(mut lock) = self.items.lock() {
            lock.push(item);
        }
    }

    /// Dequeues the next item awaiting review.
    #[must_use]
    pub fn dequeue(&self) -> Option<TriageItem> {
        let mut lock = self.items.lock().ok()?;
        if lock.is_empty() {
            None
        } else {
            Some(lock.remove(0))
        }
    }

    /// Current number of items awaiting review.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.items.lock().map(|lock| lock.len()).unwrap_or(0)
    }
}
