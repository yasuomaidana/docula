//! # docula-domains
//!
//! Standard domain vertical schemas, arithmetic integrity policies ([`DocuValidate`]),
//! and financial optical code adapters for the `docula` document intelligence engine.

#[cfg(feature = "invoice")]
pub mod invoice;

#[cfg(feature = "invoice")]
pub use invoice::{InvoiceData, LineItem, PaymentInfo};
