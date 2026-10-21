use std::path::PathBuf;
use thiserror::Error;

/// Top-level error type for core docula operations.
#[derive(Debug, Error)]
pub enum DoculaError {
    /// Spatial geometry computation or validation failure.
    #[error("Geometry error: {0}")]
    Geometry(#[from] GeometryError),

    /// Document input source ingestion or reading failure.
    #[error("Document source error: {0}")]
    Source(#[from] SourceError),

    /// Image cropping or screenshot extraction failure.
    #[error("Crop error: {0}")]
    Crop(#[from] CropError),

    /// Configuration loading or validation failure.
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// Underlying standard I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors related to spatial coordinate calculations and validations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GeometryError {
    /// Coordinates are outside the normalized [0.0, 1.0] unit square.
    #[error("Coordinate out of normalized range [0.0, 1.0]: {msg}")]
    OutOfBounds { msg: String },

    /// The bounding box has an inverted coordinate order (e.g. x0 > x1 or y0 > y1).
    #[error("Inverted bounding box bounds: {msg}")]
    InvertedBounds { msg: String },

    /// A polygon has fewer than 3 vertices or degenerate coordinates.
    #[error("Degenerate polygon: {msg}")]
    DegeneratePolygon { msg: String },

    /// Image dimensions provided for pixel projection are zero.
    #[error("Image dimensions must be non-zero for pixel projection (got {width}x{height})")]
    ZeroImageDimensions { width: u32, height: u32 },

    /// Confidence score is outside the valid probability interval [0.0, 1.0].
    #[error("Confidence score must be within [0.0, 1.0] (got {score})")]
    InvalidConfidence { score: String },
}

/// Errors related to document input payload inspection.
#[derive(Debug, Error)]
pub enum SourceError {
    /// Ingested payload contains zero bytes.
    #[error("Input payload is empty")]
    EmptyPayload,

    /// Document file does not exist at specified path.
    #[error("Document file not found at: {path}")]
    FileNotFound { path: PathBuf },

    /// Unrecognized document format or mime type.
    #[error("Unsupported or unrecognized document format: {format}")]
    UnsupportedFormat { format: String },
}

/// Errors related to visual region cropping and rendering.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CropError {
    /// Cropped sub-image dimension is zero.
    #[error("Cropped region has zero dimensions ({width}x{height})")]
    ZeroDimensions { width: u32, height: u32 },

    /// Requested page index is beyond the bounds of the document.
    #[error("Page index {page_index} out of bounds (document has {total_pages} pages)")]
    PageOutOfBounds {
        page_index: usize,
        total_pages: usize,
    },

    /// Image encoding or base64 serialization failure.
    #[error("Image encoding failed: {reason}")]
    EncodingFailed { reason: String },
}

/// Errors related to consumer configuration loading and validation.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The TOML document is malformed or contains unknown / mistyped keys.
    #[error("Failed to parse configuration: {reason}")]
    ParseFailure { reason: String },

    /// The configuration file could not be read.
    #[error("Failed to read configuration file {path}: {source}")]
    FileRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// A configuration value violates a documented invariant.
    #[error("Invalid configuration value for `{field}`: {reason}")]
    InvalidValue { field: &'static str, reason: String },

    /// The configuration could not be serialized.
    #[error("Failed to serialize configuration: {reason}")]
    SerializeFailure { reason: String },
}
