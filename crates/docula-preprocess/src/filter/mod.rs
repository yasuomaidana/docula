//! Modular image filter pipeline, computer vision transformations, and OCR tensor preparation.
//!
//! This module provides:
//! - [`ImageFilter`]: Strategy trait for modular image filter passes.
//! - [`PreprocessingFilterChain`]: Chain of Responsibility pipeline executing filter passes sequentially.
//! - [`HoughDeskewFilter`]: Dominant orientation detection via Hough line transform and affine deskew rotation.
//! - [`AdaptiveBinarizeFilter`]: Local window adaptive thresholding for shadow removal and high contrast.
//! - [`SimdResizeFilter`]: SIMD-accelerated image scaling preserving aspect ratio.
//! - [`OxiMediaFilter`]: KitaSan sovereign OpenCV replacement filter for contrast and luminance enhancement.
//! - [`OxiMediaStitcher`]: Pure-Rust multi-image vertical seam stitcher and alpha blender.
//! - [`PreprocessedImage`]: Normalized output structure containing processed buffers and OCR-ready tensors.

pub mod binarize;
pub mod chain;
pub mod deskew;
pub mod oximedia;
pub mod preprocessed;
pub mod resize;

pub use binarize::AdaptiveBinarizeFilter;
pub use chain::PreprocessingFilterChain;
pub use deskew::HoughDeskewFilter;
pub use oximedia::{OxiMediaFilter, OxiMediaStitcher};
pub use preprocessed::PreprocessedImage;
pub use resize::{ResizeMode, SimdResizeFilter};

use crate::error::PreprocessError;
use image::RgbaImage;

/// Common interface for composable image processing filters.
///
/// Implements the Strategy Pattern: each filter encapsulates an image transformation
/// algorithm (e.g. deskewing, binarization, rescaling) that can be plugged into a
/// [`PreprocessingFilterChain`].
pub trait ImageFilter: Send + Sync {
    /// Human-readable name of the filter for telemetry and debugging.
    fn name(&self) -> &'static str;

    /// Applies the transformation to the provided RGBA image in place.
    ///
    /// # Errors
    /// Returns [`PreprocessError`] if the transformation fails.
    fn apply(&self, img: &mut RgbaImage) -> Result<(), PreprocessError>;
}
