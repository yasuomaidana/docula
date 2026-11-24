//! # docula-ingest
//!
//! PDF object tree inspection, font ToUnicode mapping validation, and intelligent
//! routing heuristics for the `docula` document intelligence engine.
//!
//! This crate provides:
//! - [`DocumentRouter`]: Central orchestrator routing document sources into born-digital or OCR pipelines.
//! - [`PdfInspector`]: Low-level PDF object tree, font descriptor, and `/ToUnicode` stream validator.
//! - [`OfficeInspector`]: Pure-Rust parser and router for spreadsheets, presentations, and word documents.
//! - [`PageRoute`]: Enumeration of per-page routing decisions ([`PageRoute::BornDigital`], [`PageRoute::NeedsOcr`]).
//! - [`RasterJob`]: Rasterization execution parameters for scanned pages.
//! - [`DocumentRoutingReport`]: Comprehensive multi-page routing summary.
//! - [`RouterConfig`]: Heuristic parameters and character count thresholds.
//! - [`IngestError`]: Strongly typed ingestion error hierarchy.

pub mod error;
pub mod inspector;
pub mod office;
pub mod router;
pub mod types;

pub use error::IngestError;
pub use inspector::PdfInspector;
pub use office::{
    OfficeInspector, PresentationDeck, PresentationReader, PresentationSlide, SpreadsheetCell,
    SpreadsheetReader, SpreadsheetSheet, SpreadsheetWorkbook, WordDocument, WordProcessingReader,
};
pub use router::DocumentRouter;
pub use types::{DocumentRoutingReport, PageRoute, RasterJob, RouterConfig};
