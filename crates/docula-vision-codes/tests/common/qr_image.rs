//! Renders encoded barcode matrices as images for the optical-code tests.
//!
//! Included with `#[path]` by the vision-codes, CLI and workspace-root test targets.

use image::{Rgba, RgbaImage};
use rxing::common::BitMatrix;

/// Draws `matrix` as black modules on a white background, one pixel per module.
pub fn bitmatrix_to_rgba(matrix: &BitMatrix) -> RgbaImage {
    RgbaImage::from_fn(matrix.getWidth(), matrix.getHeight(), |x, y| {
        if matrix.get(x, y) {
            Rgba([0, 0, 0, 255])
        } else {
            Rgba([255, 255, 255, 255])
        }
    })
}
