use thiserror::Error;

/// Errors arising during document reconstruction or instance segmentation.
#[derive(Error, Debug)]
pub enum ComposerError {
    /// Provided fragment list was empty.
    #[error("Empty fragments provided for document reconstruction")]
    EmptyFragments,

    /// Fragment loading or dimension error.
    #[error("Document fragment error: {0}")]
    Fragment(String),

    /// Image registration or alignment failure.
    #[error("Feature alignment failed: {0}")]
    AlignmentFailed(String),

    /// Image codec or buffer conversion failure.
    #[error("Image processing error: {0}")]
    ImageError(String),

    /// Preprocessing or filter pipeline failure.
    #[error("Preprocessing failure: {0}")]
    Preprocessing(String),

    /// Multi-document instance segmentation failure.
    #[error("Instance segmentation failed: {0}")]
    SegmentationFailed(String),

    /// Optical code or barcode detection error.
    #[error("Optical code extraction failure: {0}")]
    OpticalCode(String),

    /// Standard I/O failure.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
