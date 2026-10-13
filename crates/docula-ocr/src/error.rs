use docula_core::{DoculaError, GeometryError};
use std::path::PathBuf;
use thiserror::Error;

/// Errors arising during OCR detection, recognition, and asset loading.
#[derive(Debug, Error)]
pub enum OcrError {
    /// Ingested image has empty or invalid dimensions.
    #[error("Image buffer is empty (dimensions: {width}x{height})")]
    EmptyImage {
        /// Image width in pixels.
        width: u32,
        /// Image height in pixels.
        height: u32,
    },

    /// Required neural network model or dictionary asset was not found.
    #[error("OCR model asset not found at '{path}': {reason}")]
    ModelNotFound {
        /// File path where model was looked up.
        path: PathBuf,
        /// Reason description.
        reason: String,
    },

    /// Error during neural text detection (DBNet).
    #[error("Text detection failure: {0}")]
    DetectionFailure(String),

    /// Error during character sequence recognition (CRNN).
    #[error("Text recognition failure: {0}")]
    RecognitionFailure(String),

    /// General neural inference failure.
    #[error("Neural inference failure: {0}")]
    InferenceFailure(String),

    /// Memory mapping failure.
    #[error("Memory mapping error for '{path}': {reason}")]
    MmapError {
        /// Path of the mapped file.
        path: PathBuf,
        /// Error details.
        reason: String,
    },

    /// Spatial geometry computation or validation failure.
    #[error("Geometry error: {0}")]
    Geometry(#[from] GeometryError),

    /// I/O error propagated during model loading.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Core domain error propagated from primitives.
    #[error("Core error: {0}")]
    Core(#[from] DoculaError),
}
