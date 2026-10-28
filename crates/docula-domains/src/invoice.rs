//! Invoicing and Accounts Payable domain schema, macros, and validation.

pub use docula_schema::DocuValidate;
pub use docula_schema::ValidationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "vision-codes")]
pub use docula_vision_codes::types::{MexicanSatCfdiPayload, SepaEpcPayload, SwissQrPayload};

/// Strategy defining where discount deductions apply within an invoice.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub enum DiscountPolicy {
    None,
    #[default]
    Invoice,
    LineItem,
    Both,
}

/// Details of an individual item or service line within an invoice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LineItem {
    #[schemars(description = "Description or line name of the item/service")]
    pub description: String,

    #[schemars(description = "Quantity purchased or hours worked")]
    pub quantity: f64,

    #[schemars(description = "Price per unit")]
    pub unit_price: f64,

    #[schemars(description = "Optional line-level discount deduction")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount: Option<f64>,

    #[schemars(description = "Line total amount after line discounts")]
    pub total: f64,
}

impl LineItem {
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
    #[schemars(description = "Target IBAN account number")]
    pub iban: Option<String>,

    #[schemars(description = "SWIFT / BIC code of receiving bank")]
    pub bic: Option<String>,

    #[schemars(description = "Beneficiary recipient name")]
    pub beneficiary_name: Option<String>,

    #[schemars(description = "Payment reference or remittance descriptor")]
    pub reference: Option<String>,
}

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

/// Macro synthesizing customizable invoice schemas with standard domain fields,
/// custom user fields, flexible discount strategies, and automatic `DocuValidate` math checks.
#[macro_export]
macro_rules! docula_invoice {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            discount_policy = both,
            $($(#[$field_meta:meta])* $field_vis:vis $field_name:ident : $field_ty:ty),* $(,)?
        }
    ) => {
        $crate::docula_invoice!(@internal_emit
            $(#[$meta])*
            $vis struct $name {
                optional_discount: [
                    #[schemars(description = "Discount deducted from subtotal")]
                    pub discount_amount: Option<f64>,
                ],
                has_discount: true,
                custom_fields: [$($(#[$field_meta])* $field_vis $field_name : $field_ty),*]
            }
        );
    };

    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            discount_policy = invoice,
            $($(#[$field_meta:meta])* $field_vis:vis $field_name:ident : $field_ty:ty),* $(,)?
        }
    ) => {
        $crate::docula_invoice!(@internal_emit
            $(#[$meta])*
            $vis struct $name {
                optional_discount: [
                    #[schemars(description = "Discount deducted from subtotal")]
                    pub discount_amount: Option<f64>,
                ],
                has_discount: true,
                custom_fields: [$($(#[$field_meta])* $field_vis $field_name : $field_ty),*]
            }
        );
    };

    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            discount_policy = line_item,
            $($(#[$field_meta:meta])* $field_vis:vis $field_name:ident : $field_ty:ty),* $(,)?
        }
    ) => {
        $crate::docula_invoice!(@internal_emit
            $(#[$meta])*
            $vis struct $name {
                optional_discount: [],
                has_discount: false,
                custom_fields: [$($(#[$field_meta])* $field_vis $field_name : $field_ty),*]
            }
        );
    };

    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            discount_policy = none,
            $($(#[$field_meta:meta])* $field_vis:vis $field_name:ident : $field_ty:ty),* $(,)?
        }
    ) => {
        $crate::docula_invoice!(@internal_emit
            $(#[$meta])*
            $vis struct $name {
                optional_discount: [],
                has_discount: false,
                custom_fields: [$($(#[$field_meta])* $field_vis $field_name : $field_ty),*]
            }
        );
    };

    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $($(#[$field_meta:meta])* $field_vis:vis $field_name:ident : $field_ty:ty),* $(,)?
        }
    ) => {
        $crate::docula_invoice!(@internal_emit
            $(#[$meta])*
            $vis struct $name {
                optional_discount: [
                    #[schemars(description = "Discount deducted from subtotal")]
                    pub discount_amount: Option<f64>,
                ],
                has_discount: true,
                custom_fields: [$($(#[$field_meta])* $field_vis $field_name : $field_ty),*]
            }
        );
    };

    (@internal_emit
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            optional_discount: [$($(#[$disc_meta:meta])* pub $disc_field:ident : $disc_ty:ty,)*],
            has_discount: $has_disc:tt,
            custom_fields: [$($(#[$field_meta:meta])* $field_vis:vis $field_name:ident : $field_ty:ty),*]
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

            $($(#[$disc_meta])* pub $disc_field : $disc_ty,)*

            #[schemars(description = "Grand total amount due")]
            pub total_amount: f64,

            #[schemars(description = "Remittance payment instructions")]
            pub payment_info: Option<$crate::invoice::PaymentInfo>,

            $($(#[$field_meta])* $field_vis $field_name: $field_ty,)*
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

                let disc_val = $crate::docula_invoice!(@get_discount self, $has_disc);
                if let Err(e) = $crate::invoice::validate_invoice_totals(
                    self.subtotal_amount,
                    self.tax_amount,
                    disc_val,
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

    (@get_discount $self:ident, true) => {
        $self.discount_amount
    };

    (@get_discount $self:ident, false) => {
        None
    };

    (@adapters $name:ident) => {
        #[cfg(feature = "vision-codes")]
        impl $name {
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

docula_invoice! {
    /// Standardized Accounts Payable invoice model with built-in arithmetic validation.
    pub struct InvoiceData {
        discount_policy = invoice,
    }
}
