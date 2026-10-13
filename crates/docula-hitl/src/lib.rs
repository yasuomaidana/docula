//! # docula-hitl
//!
//! Human-in-the-loop (HITL) triage gates, immutable correction triad capture,
//! two-tier storage, active learning promotion engine, and recursive lineage DAG tracking for `docula`.

pub mod error;
pub mod lineage;
pub mod promotion;
pub mod storage;
pub mod triad;
pub mod triage;

pub use error::HitlError;
pub use lineage::{CorrectionLineageTracker, LineageRevision};
pub use promotion::PromotionGate;
pub use storage::ColdArchiveStore;
pub use triad::{CorrectionTriad, JsonPatchDiff, TriadMetadata};
pub use triage::{TriageDecision, TriageGate, TriageItem, TriageQueue};
