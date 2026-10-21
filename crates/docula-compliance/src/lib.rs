//! # docula-compliance
//!
//! Compliance action tags (`@censor`, `@ignore`, `@non-storing`), spatial pixel blackout,
//! and deterministic PII sanitization for docula.

pub mod directive;
pub mod error;
pub mod pattern;
pub mod redactor;

pub use directive::ActionDirective;
pub use error::ComplianceError;
pub use pattern::CensorPattern;
pub use redactor::ComplianceRedactor;
