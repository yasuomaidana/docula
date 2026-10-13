//! Receipt and Travel/Expense Normalization domain schema, macros, and validation.
//!
//! Provides the [`docula_receipt!`] extensible macro, composable [`ReceiptItem`],
//! and the standard [`ReceiptData`] preset with OCR jitter tolerance and corporate expense tracking.
//! Implements arithmetic consistency checks via [`docula_schema::DocuValidate`].

pub use docula_schema::DocuValidate;
use docula_schema::ValidationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An item or service purchased on a receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReceiptItem {
    /// Description or line name of the item.
    #[schemars(description = "Description or name of item")]
    pub description: String,

    /// Quantity purchased if itemized.
    #[schemars(description = "Quantity purchased")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<f64>,

    /// Unit price per item if itemized.
    #[schemars(description = "Unit price per item")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_price: Option<f64>,

    /// Total price for this line item.
    #[schemars(description = "Line item total amount")]
    pub total: f64,

    /// Optional categorization (e.g. "Meals", "Transport", "Supplies").
    #[schemars(description = "Item expense category")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

impl ReceiptItem {
    /// Creates a simple receipt line item with just description and total.
    #[must_use]
    pub fn new(description: impl Into<String>, total: f64) -> Self {
        Self {
            description: description.into(),
            quantity: None,
            unit_price: None,
            total: (total * 100.0).round() / 100.0,
            category: None,
        }
    }

    /// Creates an itemized receipt line item with quantity and unit price.
    #[must_use]
    pub fn itemized(description: impl Into<String>, quantity: f64, unit_price: f64) -> Self {
        let total = (quantity * unit_price * 100.0).round() / 100.0;
        Self {
            description: description.into(),
            quantity: Some(quantity),
            unit_price: Some(unit_price),
            total,
            category: None,
        }
    }
}

// =========================================================================
// Modular Complexity-Bounded Integrity Validators (CC <= 6, Cognitive <= 6)
// =========================================================================

/// Validates receipt math: subtotal + tax + tip == grand total within tolerance.
pub fn validate_receipt_totals(
    subtotal: Option<f64>,
    tax: Option<f64>,
    tip: Option<f64>,
    total: f64,
    tolerance: f64,
) -> Result<(), ValidationError> {
    if let Some(sub) = subtotal {
        let calculated = sub + tax.unwrap_or(0.0) + tip.unwrap_or(0.0);
        if (calculated - total).abs() > tolerance {
            return Err(ValidationError {
                field_path: "total_amount".to_string(),
                error_message: format!(
                    "Calculated total ({:.2} = subtotal {:.2} + tax {:.2} + tip {:.2}) does not match total_amount ({:.2})",
                    calculated,
                    sub,
                    tax.unwrap_or(0.0),
                    tip.unwrap_or(0.0),
                    total
                ),
            });
        }
    }
    Ok(())
}

/// Validates sum of itemized receipt items matches subtotal if both are present.
pub fn validate_receipt_items_sum(
    items: &[ReceiptItem],
    subtotal: Option<f64>,
    tolerance: f64,
) -> Result<(), ValidationError> {
    if !items.is_empty()
        && let Some(sub) = subtotal
    {
        let items_sum: f64 = items.iter().map(|item| item.total).sum();
        if (items_sum - sub).abs() > tolerance {
            return Err(ValidationError {
                field_path: "subtotal_amount".to_string(),
                error_message: format!(
                    "Sum of itemized lines ({:.2}) does not match subtotal_amount ({:.2})",
                    items_sum, sub
                ),
            });
        }
    }
    Ok(())
}

// =========================================================================
// Macro Generator: docula_receipt!
// =========================================================================

/// Macro synthesizing customizable receipt schemas with standard merchant fields,
/// configurable OCR jitter tolerance, and enterprise expense classification fields.
#[macro_export]
macro_rules! docula_receipt {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_ty:ty
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema)]
        $vis struct $name {
            #[schemars(description = "Merchant or establishment name")]
            pub merchant_name: String,

            #[schemars(description = "Merchant tax registration number or VAT ID")]
            pub merchant_tax_id: Option<String>,

            #[schemars(description = "Physical address or store location")]
            pub merchant_address: Option<String>,

            #[schemars(description = "Transaction date (YYYY-MM-DD)")]
            pub transaction_date: String,

            #[schemars(description = "Transaction time (HH:MM or HH:MM:SS)")]
            pub transaction_time: Option<String>,

            #[schemars(description = "ISO-4217 3-letter currency code (e.g. USD, EUR)")]
            pub currency: String,

            #[schemars(description = "Subtotal before tax and tip")]
            pub subtotal_amount: Option<f64>,

            #[schemars(description = "Tax or VAT amount")]
            pub tax_amount: Option<f64>,

            #[schemars(description = "Gratuity or tip amount")]
            pub tip_amount: Option<f64>,

            #[schemars(description = "Grand total paid")]
            pub total_amount: f64,

            #[schemars(description = "Payment method (e.g. Cash, Visa, Mastercard)")]
            pub payment_method: Option<String>,

            #[schemars(description = "List of purchased items")]
            pub items: Vec<$crate::receipt::ReceiptItem>,

            $(
                $(#[$field_meta])*
                $field_vis $field_name: $field_ty,
            )*
        }

        impl $crate::docula_schema::DocuValidate for $name {
            fn validate_integrity(&self) -> Result<(), Vec<$crate::docula_schema::ValidationError>> {
                let mut errors = Vec::new();
                const TOLERANCE: f64 = 0.08; // OCR jitter tolerance

                if let Err(e) = $crate::receipt::validate_receipt_totals(
                    self.subtotal_amount,
                    self.tax_amount,
                    self.tip_amount,
                    self.total_amount,
                    TOLERANCE,
                ) {
                    errors.push(e);
                }

                if let Err(e) = $crate::receipt::validate_receipt_items_sum(
                    &self.items,
                    self.subtotal_amount,
                    TOLERANCE,
                ) {
                    errors.push(e);
                }

                if let Err(e) = $crate::invoice::validate_iso_currency(&self.currency) {
                    errors.push(e);
                }

                if self.merchant_name.trim().is_empty() {
                    errors.push($crate::docula_schema::ValidationError {
                        field_path: "merchant_name".to_string(),
                        error_message: "Merchant name must not be empty".to_string(),
                    });
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            }
        }
    };
}

// Preset standard ReceiptData instantiation
docula_receipt! {
    /// Standardized receipt schema with OCR jitter tolerance and tip/tax normalization.
    pub struct ReceiptData {}
}
