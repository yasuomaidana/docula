//! Invoicing and Accounts Payable domain schema, macros, and validation.
//!
//! Provides the [`docula_invoice!`] extensible macro, composable [`LineItem`],
//! [`PaymentInfo`], and the standard [`InvoiceData`] preset.
//! Implements strict arithmetic integrity checks via [`docula_schema::DocuValidate`]
//! and optical code adapters (SEPA EPC, SAT CFDI, Swiss QR).

pub use docula_schema::DocuValidate;
use docula_schema::ValidationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "vision-codes")]
pub use docula_vision_codes::types::{MexicanSatCfdiPayload, SepaEpcPayload, SwissQrPayload};

/// Strategy defining where discount deductions apply within an invoice.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub enum DiscountPolicy {
    /// No discounts allowed or expected.
    None,
    /// Invoice/document-level discount deduction only (`discount_amount`).
    #[default]
    Invoice,
    /// Item-level discount deductions only (`LineItem.discount`).
    LineItem,
    /// Dual discounts: item-level deductions and document-level deductions.
    Both,
}

/// Details of an individual item or service line within an invoice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LineItem {
    /// Description or title of the product or service.
    #[schemars(description = "Description or line name of the item/service")]
    pub description: String,

    /// Quantity billed.
    #[schemars(description = "Quantity purchased or hours worked")]
    pub quantity: f64,

    /// Unit price per unit of quantity.
    #[schemars(description = "Price per unit")]
    pub unit_price: f64,

    /// Optional item-level discount deduction applied to this line.
    #[schemars(description = "Optional line-level discount deduction")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount: Option<f64>,

    /// Total price for this line item (typically `(quantity * unit_price) - discount`).
    #[schemars(description = "Line total amount after line discounts")]
    pub total: f64,
}

impl LineItem {
    /// Creates a new `LineItem` with calculated total and no line discount.
    #[must_use]
    pub fn new(description: impl Into<String>, quantity: f64, unit_price: f64) -> Self {
        let total = (quantity * unit_price * 100.0).round() / 100.0;
        Self {
            description: description.into(),
            quantity,
            unit_price,
            discount: None,
            total,
        }
    }

    /// Creates a new `LineItem` with an explicit line-level discount deduction.
    #[must_use]
    pub fn with_discount(
        description: impl Into<String>,
        quantity: f64,
        unit_price: f64,
        discount: f64,
    ) -> Self {
        let raw = (quantity * unit_price) - discount;
        let total = (raw * 100.0).round() / 100.0;
        Self {
            description: description.into(),
            quantity,
            unit_price,
            discount: Some(discount),
            total,
        }
    }
}

/// Remittance and payment destination details.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PaymentInfo {
    /// International Bank Account Number (IBAN).
    #[schemars(description = "Target IBAN account number")]
    pub iban: Option<String>,

    /// Bank Identifier Code (BIC / SWIFT).
    #[schemars(description = "SWIFT / BIC code of receiving bank")]
    pub bic: Option<String>,

    /// Legal entity or beneficiary recipient name.
    #[schemars(description = "Beneficiary recipient name")]
    pub beneficiary_name: Option<String>,

    /// Remittance reference or payment purpose text.
    #[schemars(description = "Payment reference or remittance descriptor")]
    pub reference: Option<String>,
}

// =========================================================================
// Modular Complexity-Bounded Integrity Validators (CC <= 5, Cognitive <= 5)
// =========================================================================

/// Validates individual line item math within tolerance: `(quantity * unit_price) - discount == total`.
pub fn validate_line_item(
    item: &LineItem,
    index: usize,
    tolerance: f64,
) -> Result<(), ValidationError> {
    let disc = item.discount.unwrap_or(0.0);
    let expected_total = (item.quantity * item.unit_price) - disc;
    if (expected_total - item.total).abs() > tolerance {
        Err(ValidationError {
            field_path: format!("line_items[{}].total", index),
            error_message: format!(
                "Item {} total ({:.2}) does not match (quantity {} * unit_price {:.2}) - discount ({:.2}) = {:.2}",
                item.description, item.total, item.quantity, item.unit_price, disc, expected_total
            ),
        })
    } else {
        Ok(())
    }
}

/// Validates sum of line item totals equals the stated subtotal within tolerance.
pub fn validate_line_items_sum(
    items: &[LineItem],
    subtotal: f64,
    tolerance: f64,
) -> Result<(), ValidationError> {
    if !items.is_empty() {
        let items_sum: f64 = items.iter().map(|item| item.total).sum();
        if (items_sum - subtotal).abs() > tolerance {
            return Err(ValidationError {
                field_path: "subtotal_amount".to_string(),
                error_message: format!(
                    "Sum of line items ({:.2}) does not match subtotal_amount ({:.2})",
                    items_sum, subtotal
                ),
            });
        }
    }
    Ok(())
}

/// Validates grand total calculation: `subtotal + tax - discount == total` within tolerance.
pub fn validate_invoice_totals(
    subtotal: f64,
    tax: f64,
    discount: Option<f64>,
    total: f64,
    tolerance: f64,
) -> Result<(), ValidationError> {
    let disc = discount.unwrap_or(0.0);
    let calculated_total = subtotal + tax - disc;
    if (calculated_total - total).abs() > tolerance {
        Err(ValidationError {
            field_path: "total_amount".to_string(),
            error_message: format!(
                "Calculated total ({:.2} = subtotal {:.2} + tax {:.2} - discount {:.2}) does not match total_amount ({:.2})",
                calculated_total, subtotal, tax, disc, total
            ),
        })
    } else {
        Ok(())
    }
}

/// Validates currency conforms to 3-letter uppercase ISO-4217 standard.
pub fn validate_iso_currency(currency: &str) -> Result<(), ValidationError> {
    let trimmed = currency.trim();
    if trimmed.len() != 3 || !trimmed.chars().all(|c| c.is_ascii_uppercase()) {
        Err(ValidationError {
            field_path: "currency".to_string(),
            error_message: format!(
                "Currency {} must be a valid 3-letter uppercase ISO-4217 code (e.g. USD, EUR)",
                currency
            ),
        })
    } else {
        Ok(())
    }
}

/// Validates vendor name is non-empty.
pub fn validate_non_empty_vendor(vendor_name: &str) -> Result<(), ValidationError> {
    if vendor_name.trim().is_empty() {
        Err(ValidationError {
            field_path: "vendor_name".to_string(),
            error_message: "Vendor name must not be empty".to_string(),
        })
    } else {
        Ok(())
    }
}

// =========================================================================
// Macro Generator: docula_invoice!
// =========================================================================

/// Macro synthesizing customizable invoice schemas with standard domain fields,
/// custom user fields, flexible discount strategies, and automatic `DocuValidate` math checks.
#[macro_export]
macro_rules! docula_invoice {
    // Branch 1: Explicit discount_policy = both
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            discount_policy = both,
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_ty:ty
            ),* $(,)?
        }
    ) => {
        $crate::docula_invoice!(@internal_emit
            $(#[$meta])*
            $vis struct $name {
                has_invoice_discount: true,
                custom_fields: [
                    $($(#[$field_meta])* $field_vis $field_name : $field_ty),*
                ]
            }
        );
    };

    // Branch 2: Explicit discount_policy = invoice
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            discount_policy = invoice,
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_ty:ty
            ),* $(,)?
        }
    ) => {
        $crate::docula_invoice!(@internal_emit
            $(#[$meta])*
            $vis struct $name {
                has_invoice_discount: true,
                custom_fields: [
                    $($(#[$field_meta])* $field_vis $field_name : $field_ty),*
                ]
            }
        );
    };

    // Branch 3: Explicit discount_policy = line_item
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            discount_policy = line_item,
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_ty:ty
            ),* $(,)?
        }
    ) => {
        $crate::docula_invoice!(@internal_emit
            $(#[$meta])*
            $vis struct $name {
                has_invoice_discount: false,
                custom_fields: [
                    $($(#[$field_meta])* $field_vis $field_name : $field_ty),*
                ]
            }
        );
    };

    // Branch 4: Explicit discount_policy = none
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            discount_policy = none,
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_ty:ty
            ),* $(,)?
        }
    ) => {
        $crate::docula_invoice!(@internal_emit
            $(#[$meta])*
            $vis struct $name {
                has_invoice_discount: false,
                custom_fields: [
                    $($(#[$field_meta])* $field_vis $field_name : $field_ty),*
                ]
            }
        );
    };

    // Branch 5: Default (no explicit discount_policy specified)
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_ty:ty
            ),* $(,)?
        }
    ) => {
        $crate::docula_invoice!(@internal_emit
            $(#[$meta])*
            $vis struct $name {
                has_invoice_discount: true,
                custom_fields: [
                    $($(#[$field_meta])* $field_vis $field_name : $field_ty),*
                ]
            }
        );
    };

    // Internal emitter: with invoice discount
    (@internal_emit
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            has_invoice_discount: true,
            custom_fields: [
                $($(#[$field_meta:meta])* $field_vis:vis $field_name:ident : $field_ty:ty),*
            ]
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema)]
        $vis struct $name {
            #[schemars(description = "Legal vendor or merchant name")]
            pub vendor_name: String,

            #[schemars(description = "Vendor Tax ID / VAT Number")]
            pub vendor_tax_id: Option<String>,

            #[schemars(description = "Invoice identification number")]
            pub invoice_number: Option<String>,

            #[schemars(description = "Invoice issue date (YYYY-MM-DD)")]
            pub issue_date: String,

            #[schemars(description = "Invoice due date (YYYY-MM-DD)")]
            pub due_date: Option<String>,

            #[schemars(description = "ISO-4217 currency code (e.g. USD, EUR)")]
            pub currency: String,

            #[schemars(description = "List of billed line items")]
            pub line_items: Vec<$crate::invoice::LineItem>,

            #[schemars(description = "Net invoice subtotal before taxes and discounts")]
            pub subtotal_amount: f64,

            #[schemars(description = "Total taxes applied")]
            pub tax_amount: f64,

            #[schemars(description = "Discount deducted from subtotal")]
            pub discount_amount: Option<f64>,

            #[schemars(description = "Grand total amount due")]
            pub total_amount: f64,

            #[schemars(description = "Remittance payment instructions")]
            pub payment_info: Option<$crate::invoice::PaymentInfo>,

            $(
                $(#[$field_meta])*
                $field_vis $field_name: $field_ty,
            )*
        }

        impl $crate::docula_schema::DocuValidate for $name {
            fn validate_integrity(&self) -> Result<(), Vec<$crate::docula_schema::ValidationError>> {
                let mut errors = Vec::new();
                const TOLERANCE: f64 = 0.05;

                for (i, item) in self.line_items.iter().enumerate() {
                    if let Err(e) = $crate::invoice::validate_line_item(item, i, TOLERANCE) {
                        errors.push(e);
                    }
                }

                if let Err(e) = $crate::invoice::validate_line_items_sum(&self.line_items, self.subtotal_amount, TOLERANCE) {
                    errors.push(e);
                }

                if let Err(e) = $crate::invoice::validate_invoice_totals(
                    self.subtotal_amount,
                    self.tax_amount,
                    self.discount_amount,
                    self.total_amount,
                    TOLERANCE,
                ) {
                    errors.push(e);
                }

                if let Err(e) = $crate::invoice::validate_iso_currency(&self.currency) {
                    errors.push(e);
                }

                if let Err(e) = $crate::invoice::validate_non_empty_vendor(&self.vendor_name) {
                    errors.push(e);
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            }
        }

        $crate::docula_invoice!(@adapters $name);
    };

    // Internal emitter: without invoice discount
    (@internal_emit
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            has_invoice_discount: false,
            custom_fields: [
                $($(#[$field_meta:meta])* $field_vis:vis $field_name:ident : $field_ty:ty),*
            ]
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema)]
        $vis struct $name {
            #[schemars(description = "Legal vendor or merchant name")]
            pub vendor_name: String,

            #[schemars(description = "Vendor Tax ID / VAT Number")]
            pub vendor_tax_id: Option<String>,

            #[schemars(description = "Invoice identification number")]
            pub invoice_number: Option<String>,

            #[schemars(description = "Invoice issue date (YYYY-MM-DD)")]
            pub issue_date: String,

            #[schemars(description = "Invoice due date (YYYY-MM-DD)")]
            pub due_date: Option<String>,

            #[schemars(description = "ISO-4217 currency code (e.g. USD, EUR)")]
            pub currency: String,

            #[schemars(description = "List of billed line items")]
            pub line_items: Vec<$crate::invoice::LineItem>,

            #[schemars(description = "Net invoice subtotal before taxes and discounts")]
            pub subtotal_amount: f64,

            #[schemars(description = "Total taxes applied")]
            pub tax_amount: f64,

            #[schemars(description = "Grand total amount due")]
            pub total_amount: f64,

            #[schemars(description = "Remittance payment instructions")]
            pub payment_info: Option<$crate::invoice::PaymentInfo>,

            $(
                $(#[$field_meta])*
                $field_vis $field_name: $field_ty,
            )*
        }

        impl $crate::docula_schema::DocuValidate for $name {
            fn validate_integrity(&self) -> Result<(), Vec<$crate::docula_schema::ValidationError>> {
                let mut errors = Vec::new();
                const TOLERANCE: f64 = 0.05;

                for (i, item) in self.line_items.iter().enumerate() {
                    if let Err(e) = $crate::invoice::validate_line_item(item, i, TOLERANCE) {
                        errors.push(e);
                    }
                }

                if let Err(e) = $crate::invoice::validate_line_items_sum(&self.line_items, self.subtotal_amount, TOLERANCE) {
                    errors.push(e);
                }

                if let Err(e) = $crate::invoice::validate_invoice_totals(
                    self.subtotal_amount,
                    self.tax_amount,
                    None,
                    self.total_amount,
                    TOLERANCE,
                ) {
                    errors.push(e);
                }

                if let Err(e) = $crate::invoice::validate_iso_currency(&self.currency) {
                    errors.push(e);
                }

                if let Err(e) = $crate::invoice::validate_non_empty_vendor(&self.vendor_name) {
                    errors.push(e);
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            }
        }

        $crate::docula_invoice!(@adapters $name);
    };

    // Internal adapters emitter
    (@adapters $name:ident) => {
        #[cfg(feature = "vision-codes")]
        impl $name {
            /// Enriches payment details and recipient metadata from a decoded SEPA EPC QR code.
            pub fn apply_sepa_epc(&mut self, epc: &$crate::docula_vision_codes::types::SepaEpcPayload) {
                let payment = self.payment_info.get_or_insert_with($crate::invoice::PaymentInfo::default);
                payment.iban = Some(epc.iban.clone());
                if !epc.bic.is_empty() {
                    payment.bic = Some(epc.bic.clone());
                }
                payment.beneficiary_name = Some(epc.beneficiary_name.clone());
                if let Some(ref rem) = epc.remittance_text {
                    payment.reference = Some(rem.clone());
                } else if let Some(ref rem_ref) = epc.remittance_reference {
                    payment.reference = Some(rem_ref.clone());
                }

                if self.currency.is_empty() {
                    self.currency = if epc.currency.is_empty() {
                        "EUR".to_string()
                    } else {
                        epc.currency.clone()
                    };
                }
                if let Some(amt) = epc.amount
                    && self.total_amount == 0.0
                {
                    self.total_amount = amt;
                }
            }

            /// Enriches fiscal metadata from a decoded Mexican SAT CFDI electronic invoice QR code.
            pub fn apply_sat_cfdi(&mut self, cfdi: &$crate::docula_vision_codes::types::MexicanSatCfdiPayload) {
                self.currency = "MXN".to_string();
                self.total_amount = cfdi.total;
                if self.vendor_tax_id.is_none() {
                    self.vendor_tax_id = Some(cfdi.rfc_issuer.clone());
                }
                if self.invoice_number.is_none() {
                    self.invoice_number = Some(cfdi.uuid.clone());
                }
            }

            /// Enriches payment coordinates from a decoded Swiss QR-bill code.
            pub fn apply_swiss_qr(&mut self, swiss: &$crate::docula_vision_codes::types::SwissQrPayload) {
                let payment = self.payment_info.get_or_insert_with($crate::invoice::PaymentInfo::default);
                payment.iban = Some(swiss.iban.clone());
                payment.beneficiary_name = Some(swiss.creditor_name.clone());
                if let Some(ref reference) = swiss.reference {
                    payment.reference = Some(reference.clone());
                }

                if self.currency.is_empty() {
                    self.currency = swiss.currency.clone();
                }
                if let Some(amt) = swiss.amount
                    && self.total_amount == 0.0
                {
                    self.total_amount = amt;
                }
            }
        }
    };
}

// Instantiate the standard InvoiceData preset using the macro
docula_invoice! {
    /// Standardized Accounts Payable invoice model with built-in arithmetic validation.
    pub struct InvoiceData {
        discount_policy = invoice,
    }
}
