//! # oximedia-mosaic
//!
//! Pure-Rust 2D image registration, strip overlap discovery, perspective homography unwarping,
//! and multi-frame mosaic stitching.

pub mod error;
pub mod luma;
pub mod perspective;
pub mod registration;
pub mod stitch;

pub use error::MosaicError;
pub use luma::{map_rgb_channels, map_rgba_channels, rgb_to_luma_u8, rgba_to_luma_vec};
pub use perspective::{auto_dewarp_dominant_quad, unwarp_quad_rgba};
pub use registration::{OverlapAxis, detect_strip_overlap_zmad, estimate_orthogonal_drift_ecc};
pub use stitch::{stitch_frames_feathered, stitch_pair_horizontal, stitch_pair_vertical};
