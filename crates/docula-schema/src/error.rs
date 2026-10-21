use thiserror::Error;

/// Errors arising during JSON Schema compilation, DFA traversal, and logits masking.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum SchemaError {
    /// Failed to parse JSON Schema from string or reflection.
    #[error("Failed to parse JSON Schema: {0}")]
    ParseError(String),

    /// Schema construct is unsupported or malformed.
    #[error("Unsupported or malformed schema construct: {0}")]
    UnsupportedSchema(String),

    /// Invalid transition in DFA state machine.
    #[error("Invalid transition in DFA state machine: {0}")]
    InvalidTransition(String),
}
