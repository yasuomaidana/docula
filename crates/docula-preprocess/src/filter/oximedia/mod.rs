//! OxiMedia pure-Rust computer vision integration for docula preprocessing.

pub mod clahe;
pub mod denoise;
pub mod enhance;
pub mod geometry;

pub use clahe::OxiMediaClaheFilter;
pub use denoise::{
    DenoiseMode, MorphologyDenoiseConfig, MorphologyOp, MorphologyShape, OxiMediaDenoiseFilter,
};
pub use enhance::{OxiMediaFilter, UnsharpMaskConfig};
pub use geometry::{
    OxiMediaLensUndistortFilter, OxiMediaMotionDeblurFilter, OxiMediaPerspectiveDewarpFilter,
};

use crate::error::PreprocessError;
use image::RgbaImage;
use oxipix::mosaic::{stitch_frames_feathered, stitch_pair_horizontal, stitch_pair_vertical};

/// Pure-Rust image stitcher utilizing CV mosaic primitives.
pub struct OxiMediaStitcher;

impl OxiMediaStitcher {
    /// Merges two vertically overlapping document image sections into a unified canvas.
    pub fn stitch_vertical(
        top: &RgbaImage,
        bottom: &RgbaImage,
        overlap_height: u32,
    ) -> Result<RgbaImage, PreprocessError> {
        stitch_pair_vertical(top, bottom, overlap_height).map_err(|e| {
            PreprocessError::FilterExecution {
                filter: "OxiMediaStitcher",
                reason: e.to_string(),
            }
        })
    }

    /// Merges two horizontally overlapping document image sections into a unified canvas.
    pub fn stitch_horizontal(
        left: &RgbaImage,
        right: &RgbaImage,
        overlap_width: u32,
    ) -> Result<RgbaImage, PreprocessError> {
        stitch_pair_horizontal(left, right, overlap_width).map_err(|e| {
            PreprocessError::FilterExecution {
                filter: "OxiMediaStitcher",
                reason: e.to_string(),
            }
        })
    }

    /// Automatically detects vertical overlap between two consecutive document slices.
    #[must_use]
    pub fn detect_overlap_vertical(top: &RgbaImage, bottom: &RgbaImage) -> u32 {
        let (best_overlap, _) = oxipix::mosaic::detect_strip_overlap_zmad(
            top,
            bottom,
            oxipix::mosaic::OverlapAxis::Vertical,
            0.05,
            0.50,
        );
        best_overlap
    }

    /// Stitches multiple RGBA frames with pixel offsets and linear alpha feathering.
    pub fn stitch_frames_feathered(
        frames: &[(&RgbaImage, i64, i64)],
        blend_width_px: u32,
    ) -> Result<RgbaImage, PreprocessError> {
        stitch_frames_feathered(frames, blend_width_px).map_err(|e| {
            PreprocessError::FilterExecution {
                filter: "OxiMediaStitcher",
                reason: e.to_string(),
            }
        })
    }
}
