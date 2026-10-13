//! # docula-preprocess
//!
//! Pure-Rust PDF page rasterization bridge, computer vision image filters, and
//! document preprocessing pipeline for the `docula` document intelligence engine.
//!
//! This crate provides:
//! - Pure-Rust PDF rendering strategy powered by `pdf_oxide` and `tiny-skia` ([`Rasterizer`], [`TinySkiaRasterizer`]).
//! - Pure-Rust synthetic ofimatic spreadsheet/presentation rasterizer ([`OfficeRasterizer`]).
//! - In-memory RGBA page raster buffer with sub-image ROI extraction ([`PageRasterBuffer`]).
//! - Day 0 bridge to core region cropping abstractions ([`PdfRegionCropper`], [`DocumentVisualExtractor`]).
//! - Composable image filter chain pipeline ([`ImageFilter`], [`PreprocessingFilterChain`]).
//! - Pure-Rust computer vision enhancement via KitaSan `oximedia` ([`OxiMediaFilter`], [`OxiMediaDenoiseFilter`], [`OxiMediaStitcher`]).
//! - Hough transform text baseline deskewing with sub-degree accuracy ([`HoughDeskewFilter`]).
//! - Adaptive local mean and Otsu binarization for shadow removal ([`AdaptiveBinarizeFilter`]).
//! - SIMD-accelerated aspect ratio preserving image rescaler ([`SimdResizeFilter`], [`ResizeMode`]).
//! - Normalized preprocessing container with OCR planar tensor ([`PreprocessedImage`]).
//! - Typed preprocessing error hierarchy ([`PreprocessError`]).

pub mod buffer;
pub mod error;
pub mod filter;
pub mod office_raster;
pub mod raster;

pub use buffer::PageRasterBuffer;
pub use error::PreprocessError;
pub use filter::{
    AdaptiveBinarizeFilter, DenoiseMode, HoughDeskewFilter, ImageFilter, MorphologyDenoiseConfig,
    MorphologyOp, MorphologyShape, OxiMediaDenoiseFilter, OxiMediaFilter, OxiMediaStitcher,
    PreprocessedImage, PreprocessingFilterChain, ResizeMode, SimdResizeFilter, UnsharpMaskConfig,
};
pub use office_raster::OfficeRasterizer;
pub use raster::{DocumentVisualExtractor, PdfRegionCropper, Rasterizer, TinySkiaRasterizer};
