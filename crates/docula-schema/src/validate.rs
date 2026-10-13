use serde::{Deserialize, Serialize};

/// Optional integrity validation trait for domain-specific consistency checks.
///
/// If implemented, Docula runs integrity validation after deserialization and
/// automatically triggers targeted self-repair loops if validation errors are detected.
pub trait DocuValidate {
    /// Validates business rules, arithmetic consistency, or internal formatting invariants.
    fn validate_integrity(&self) -> Result<(), Vec<ValidationError>>;
}

/// Strongly typed integrity validation failure details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationError {
    /// Dot-separated field path (e.g. `items[0].total` or `financials.tax`).
    pub field_path: String,
    /// Human-readable explanation of why validation failed.
    pub error_message: String,
}

impl ValidationError {
    /// Constructs a new `ValidationError`.
    pub fn new(field_path: impl Into<String>, error_message: impl Into<String>) -> Self {
        Self {
            field_path: field_path.into(),
            error_message: error_message.into(),
        }
    }
}
