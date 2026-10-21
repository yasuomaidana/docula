//! # docula-ocr
//!
//! Multi-lingual PP-OCRv6 neural text detection (DBNet), character recognition (CRNN),
//! and spatial reading-order clustering for the `docula` document intelligence engine.
//!
//! This crate provides:
//! - [`OcrEngine`]: Abstract strategy trait for optical character recognition.
//! - [`NeuralOcrEngine`]: End-to-end multi-lingual OCR pipeline with Rayon batching.
//! - [`MockOcrEngine`]: Deterministic mock OCR engine for hermetic testing.
//! - [`DbNetDetector`]: Differentiable Binarization Network for text polygon detection and unclip.
//! - [`DetectionBox`]: Polygonal and axis-aligned bounding box detection representation.
//! - [`CrnnRecognizer`]: Multi-lingual convolutional recurrent character recognizer.
//! - [`SpatialLayoutMap`]: 2D layout map clustering text spans into lines and tables.
//! - [`TextLine`]: Horizontal text line grouping with coordinate bounding box.
//! - [`LanguageAssetManager`]: Memory-mapped asset manager for weights and dictionaries.
//! - [`OcrError`]: Strongly typed OCR error hierarchy.

pub mod assets;
pub mod crnn;
pub mod dbnet;
pub mod engine;
pub mod error;
pub mod layout;

pub use assets::LanguageAssetManager;
pub use crnn::CrnnRecognizer;
pub use dbnet::{DbNetDetector, DetectionBox};
pub use engine::{MockOcrEngine, NeuralOcrEngine, OcrEngine};
pub use error::OcrError;
pub use layout::{SpatialLayoutMap, TextLine};
