use docula_core::{CropError, DoculaError, GeometryError};
use thiserror::Error;

/// Errors arising during PDF rasterization, image processing, and preprocessing.
#[derive(Debug, Error)]
pub enum PreprocessError {
    /// Ingested PDF payload has zero bytes.
    #[error("PDF payload is empty")]
    EmptyPayload,

    /// Requested DPI is invalid (must be positive).
    #[error("DPI must be positive (got {dpi})")]
    InvalidDpi { dpi: u32 },

    /// Document parsing or structure failure.
    #[error("PDF parsing failed: {reason}")]
    ParseFailure { reason: String },

    /// Requested page index is outside the bounds of the document.
    #[error("Page index {page_index} out of bounds (document has {total_pages} pages)")]
    PageOutOfBounds {
        /// Requested page index (0-indexed).
        page_index: usize,
        /// Total number of pages in the document.
        total_pages: usize,
    },

    /// Rasterization or rendering failure.
    #[error("Rasterization failed: {reason}")]
    RasterizationFailure { reason: String },

    /// Image encoding or buffer manipulation error.
    #[error("Image encoding failed: {reason}")]
    ImageEncodingError { reason: String },

    /// Visual cropping failure propagated from core primitives.
    #[error("Crop error: {0}")]
    Crop(#[from] CropError),

    /// Spatial geometry computation or validation failure.
    #[error("Geometry error: {0}")]
    Geometry(#[from] GeometryError),

    /// General core domain error.
    #[error("Core error: {0}")]
    Core(#[from] DoculaError),

    /// Underlying standard I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = PreprocessError::EmptyPayload;
        assert_eq!(err.to_string(), "PDF payload is empty");

        let err = PreprocessError::InvalidDpi { dpi: 0 };
        assert_eq!(err.to_string(), "DPI must be positive (got 0)");

        let err = PreprocessError::PageOutOfBounds {
            page_index: 5,
            total_pages: 2,
        };
        assert_eq!(
            err.to_string(),
            "Page index 5 out of bounds (document has 2 pages)"
        );

        let err = PreprocessError::ParseFailure {
            reason: "corrupted".to_string(),
        };
        assert_eq!(err.to_string(), "PDF parsing failed: corrupted");
    }
}
