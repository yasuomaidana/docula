use thiserror::Error;

/// Errors arising during compliance directive evaluation and redaction.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ComplianceError {
    /// Invalid regular expression pattern.
    #[error("Invalid regex pattern: {0}")]
    InvalidPattern(String),

    /// Image processing failure.
    #[error("Image processing error: {0}")]
    ImageError(String),
}
