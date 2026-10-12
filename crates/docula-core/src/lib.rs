//! # docula-core
//!
//! Core primitives, spatial geometry, and foundational abstractions for the `docula`
//! pure-Rust document intelligence engine.
//!
//! This crate constitutes Layer 0 of the `docula` architecture. It exposes:
//! - Spatial coordinate primitives ([`Point`], [`PixelRect`], [`BoundingBox`], [`Polygon`], [`SpatialSpan`]).
//! - Day 0 visual crop abstractions ([`ImageCrop`], [`CropFormat`], [`RegionCropper`]).
//! - Universal document input abstractions ([`DocumentSource`]).
//! - Typed domain error hierarchies ([`DoculaError`], [`GeometryError`], [`SourceError`], [`CropError`]).

pub mod crop;
pub mod error;
pub mod geometry;
pub mod source;

pub use crop::{CropFormat, ImageCrop, RegionCropper};
pub use error::{CropError, DoculaError, GeometryError, SourceError};
pub use geometry::{BoundingBox, PixelRect, Point, Polygon, SpatialSpan};
pub use source::DocumentSource;
