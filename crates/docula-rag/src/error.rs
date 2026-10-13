use thiserror::Error;

/// Errors arising during layout chunking, vector indexing, or hybrid retrieval.
#[derive(Error, Debug)]
pub enum RagError {
    /// Chunking failure or degenerate input.
    #[error("Chunking failure: {0}")]
    ChunkingError(String),

    /// Inconsistent embedding dimension.
    #[error("Embedding dimension mismatch (expected {expected}, got {actual})")]
    DimensionMismatch {
        /// Expected vector length.
        expected: usize,
        /// Actual vector length.
        actual: usize,
    },

    /// Empty index or invalid query.
    #[error("Index error: {0}")]
    IndexError(String),
}
