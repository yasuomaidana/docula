//! # docula-schema
//!
//! JSON Schema reflection, Deterministic Finite Automaton (DFA) state machine compilation,
//! autoregressive logits masking, and domain validation for the `docula` document intelligence engine.

pub mod dfa;
pub mod directives;
pub mod error;
pub mod masker;
pub mod validate;

pub use dfa::{DfaState, SchemaDfa};
pub use directives::{DirectiveRegistry, FieldDirective};
pub use error::SchemaError;
pub use masker::DfaLogitsMasker;
pub use schemars::JsonSchema;
pub use validate::{DocuValidate, ValidationError, ValidationMode, ValidationOutcome, Validator};
