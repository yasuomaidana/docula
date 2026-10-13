use thiserror::Error;

/// Errors arising during exemplar registration, fingerprinting, or retrieval.
#[derive(Error, Debug)]
pub enum ExemplarError {
    /// Exemplar not found in pool.
    #[error("Exemplar not found: {0}")]
    NotFound(String),

    /// Serialization or deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Missing or invalid exemplar data.
    #[error("Invalid exemplar data: {0}")]
    InvalidData(String),
}
