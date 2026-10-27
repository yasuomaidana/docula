use thiserror::Error;

/// Operational errors for 2D mosaic and registration operations.
#[derive(Debug, Error)]
pub enum MosaicError {
    #[error("Invalid image dimensions: {width}x{height} ({reason})")]
    InvalidDimensions {
        width: u32,
        height: u32,
        reason: String,
    },

    #[error("Registration or homography error: {0}")]
    RegistrationError(String),

    #[error("Stitching failure: {0}")]
    StitchError(String),

    #[error("Perspective warp error: {0}")]
    PerspectiveError(String),
}
