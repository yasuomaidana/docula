use docula_schema::SchemaError;
use thiserror::Error;

/// Errors arising during LLM inference, schema masking, or response decoding.
#[derive(Error, Debug)]
pub enum LlmError {
    /// Schema or grammar compilation failure.
    #[error("Schema error: {0}")]
    Schema(#[from] SchemaError),

    /// Model inference or generation failure.
    #[error("Neural inference failure: {0}")]
    InferenceFailure(String),

    /// Model weights or session not found.
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Maximum context window exceeded.
    #[error("Context window exceeded (max: {max}, actual: {actual})")]
    ContextWindowExceeded {
        /// Maximum allowed tokens.
        max: usize,
        /// Actual prompt token count.
        actual: usize,
    },

    /// JSON serialization or deserialization failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
