//! # docula-preprocess
//!
//! Pure-Rust PDF page rasterization bridge, computer vision image filters, and
//! document preprocessing pipeline for the `docula` document intelligence engine.
//!
//! This crate provides:
//! - Pure-Rust PDF rendering strategy powered by `pdf_oxide` and `tiny-skia` ([`Rasterizer`], [`TinySkiaRasterizer`]).
//! - In-memory RGBA page raster buffer with sub-image ROI extraction ([`PageRasterBuffer`]).
//! - Day 0 bridge to core region cropping abstractions ([`PdfRegionCropper`]).
//! - Typed preprocessing error hierarchy ([`PreprocessError`]).

pub mod buffer;
pub mod error;
pub mod raster;

pub use buffer::PageRasterBuffer;
pub use error::PreprocessError;
pub use raster::{PdfRegionCropper, Rasterizer, TinySkiaRasterizer};
