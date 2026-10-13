use docula_core::DoculaError;
use thiserror::Error;

/// Errors arising during document ingestion, PDF inspection, and routing.
#[derive(Debug, Error)]
pub enum IngestError {
    /// Ingested payload is empty.
    #[error("Document payload is empty")]
    EmptyPayload,

    /// Document could not be parsed.
    #[error("Document parsing error: {0}")]
    ParseError(String),

    /// Underlying lopdf error.
    #[error("PDF structure error: {0}")]
    PdfStructure(String),

    /// Page index out of bounds.
    #[error("Page index {page_index} out of bounds (document has {total_pages} pages)")]
    PageOutOfBounds {
        /// Requested page index (0-indexed).
        page_index: usize,
        /// Total number of pages in the document.
        total_pages: usize,
    },

    /// Corrupted or invalid ToUnicode font map stream.
    #[error("Font ToUnicode stream is invalid or corrupted: {reason}")]
    CorruptedToUnicode {
        /// Detailed description of the corruption.
        reason: String,
    },

    /// I/O error occurred during ingestion.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Error propagated from core primitives.
    #[error("Core error: {0}")]
    Core(#[from] DoculaError),
}
