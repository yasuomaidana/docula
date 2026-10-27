//! # docula-core
//!
//! Core primitives, spatial geometry, and foundational abstractions for the `docula`
//! pure-Rust document intelligence engine.
//!
//! This crate constitutes Layer 0 of the `docula` architecture. It exposes:
//! - Spatial coordinate primitives ([`Point`], [`PixelRect`], [`BoundingBox`], [`Polygon`], [`SpatialSpan`]).
//! - Day 0 visual crop abstractions ([`ImageCrop`], [`CropFormat`], [`RegionCropper`]).
//! - Universal document input abstractions ([`DocumentSource`]).
//! - Office suite format abstractions ([`OfficeFormat`], [`SpreadsheetSubtype`], [`PresentationSubtype`], [`WordSubtype`]).
//! - Zero-storage RAII smart pointer ([`NonStored`]).
//! - Consumer-driven layered configuration ([`DoculaConfig`], [`DoculaConfigBuilder`]).
//! - Typed domain error hierarchies ([`DoculaError`], [`GeometryError`], [`SourceError`],
//!   [`CropError`], [`ConfigError`]).

pub mod config;
pub mod crop;
pub mod error;
pub mod geometry;
pub mod math;
pub mod non_stored;
pub mod office;
pub mod source;

pub use math::cosine_similarity;

pub use config::{
    ComplianceConfig, DoculaConfig, DoculaConfigBuilder, ExemplarConfig, GeneralConfig, HitlConfig,
    OcrConfig, OpticalCodeAction,
};
pub use crop::{CropFormat, ImageCrop, RegionCropper};
pub use error::{ConfigError, CropError, DoculaError, GeometryError, SourceError};
pub use geometry::{BoundingBox, PixelRect, Point, Polygon, SpatialSpan};
pub use non_stored::NonStored;
pub use office::{OfficeFormat, PresentationSubtype, SpreadsheetSubtype, WordSubtype};
pub use source::DocumentSource;
