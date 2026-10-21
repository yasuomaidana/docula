//! # docula-domains
//!
//! Standard domain vertical schemas, arithmetic integrity policies ([`DocuValidate`]),
//! extensible template macros, and financial optical code adapters for the `docula` document intelligence engine.

#[doc(hidden)]
pub use docula_schema;

#[cfg(feature = "vision-codes")]
#[doc(hidden)]
pub use docula_vision_codes;

#[cfg(feature = "invoice")]
pub mod invoice;
#[cfg(feature = "invoice")]
pub use invoice::{DiscountPolicy, InvoiceData, LineItem, PaymentInfo};

#[cfg(feature = "bank")]
pub mod bank;
#[cfg(feature = "bank")]
pub use bank::{BankStatementData, BankTransaction, TransactionType};

#[cfg(feature = "receipt")]
pub mod receipt;
#[cfg(feature = "receipt")]
pub use receipt::{ReceiptData, ReceiptItem};

#[cfg(feature = "logistics")]
pub mod logistics;
#[cfg(feature = "logistics")]
pub use logistics::{BillOfLadingData, CargoItem, ShippingContainer};
