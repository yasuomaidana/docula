use docula_core::{DoculaError, GeometryError};
use thiserror::Error;

/// Typed errors encountered during optical code detection, mapping, or financial parsing.
#[derive(Debug, Error)]
pub enum OpticalCodeError {
    /// Invalid image dimensions (e.g. zero width or height).
    #[error("Invalid image dimensions: width {width}, height {height}")]
    InvalidImageDimensions { width: u32, height: u32 },

    /// Failure during optical code decoding.
    #[error("Optical code decoding failed: {reason}")]
    DecodingFailure { reason: String },

    /// Failure during financial payload parsing.
    #[error("Financial QR parsing failed: {reason}")]
    FinancialParseFailure { reason: String },

    /// Geometry mapping error.
    #[error("Geometry error: {0}")]
    Geometry(#[from] GeometryError),

    /// Core engine error.
    #[error("Core error: {0}")]
    Core(#[from] DoculaError),
}
