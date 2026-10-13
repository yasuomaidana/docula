//! # docula-exemplar
//!
//! Few-shot exemplar pool management, geometric layout SimHash fingerprinting,
//! and dual-stage retrieval for the `docula` document intelligence engine.

pub mod error;
pub mod fingerprint;
pub mod prompt;
pub mod retrieval;
pub mod template;

pub use error::ExemplarError;
pub use fingerprint::{compute_layout_fingerprint, hamming_distance};
pub use prompt::FewShotPromptBuilder;
pub use retrieval::ExemplarPool;
pub use template::{ExemplarMetadata, TemplatePair};
