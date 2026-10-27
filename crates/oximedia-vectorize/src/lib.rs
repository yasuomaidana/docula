//! # oximedia-vectorize
//!
//! Sovereign, pure-Rust raster-to-vector extraction engine:
//! - Otsu automated binarization thresholding
//! - Moore-Neighbor 8-connected contour boundary tracing
//! - Ramer-Douglas-Peucker (RDP) polygonal simplification
//! - SVG `<path>` path synthesis

pub mod contour;

pub use contour::{
    SvgVectorPath, VectorizeConfig, VectorizedGraphic, otsu_threshold, perpendicular_distance,
    simplify_rdp, trace_moore_neighbor_contours, vectorize_rgba_to_svg,
};
