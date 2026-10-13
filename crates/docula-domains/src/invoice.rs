//! Invoicing and Accounts Payable domain schema and validation.
//!
//! Provides production-ready [`InvoiceData`], [`LineItem`], and [`PaymentInfo`] schemas
//! decorated with `schemars::JsonSchema` and `serde::{Serialize, Deserialize}`,
//! implementing strict arithmetic integrity checks via [`docula_schema::DocuValidate`].
//!
//! Includes adapters bridging decoded 2D barcodes (SEPA EPC QR, Swiss QR-bill, and Mexican SAT CFDI)
//! directly into invoice financial fields.

use docula_schema::{DocuValidate, ValidationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "vision-codes")]
use docula_vision_codes::types::{MexicanSatCfdiPayload, SepaEpcPayload, SwissQrPayload};

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

    /// Total price for this line item (typically `quantity * unit_price`).
    #[schemars(description = "Line total amount (quantity * unit_price)")]
    pub total: f64,
}

impl LineItem {
    /// Creates a new `LineItem` with calculated total.
    #[must_use]
    pub fn new(description: impl Into<String>, quantity: f64, unit_price: f64) -> Self {
        let total = (quantity * unit_price * 100.0).round() / 100.0;
        Self {
            description: description.into(),
            quantity,
            unit_price,
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

/// Standardized Accounts Payable invoice model with built-in arithmetic validation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InvoiceData {
    /// Legal trade or corporate name of the vendor/merchant.
    #[schemars(description = "Legal vendor or merchant name")]
    pub vendor_name: String,

    /// Vendor's tax registration or VAT identifier.
    #[schemars(description = "Vendor Tax ID / VAT Number")]
    pub vendor_tax_id: Option<String>,

    /// Unique invoice number or reference code.
    #[schemars(description = "Invoice identification number")]
    pub invoice_number: Option<String>,

    /// Issue date in ISO-8601 format (YYYY-MM-DD).
    #[schemars(description = "Invoice issue date (YYYY-MM-DD)")]
    pub issue_date: String,

    /// Due date for payment (YYYY-MM-DD).
    #[schemars(description = "Invoice due date (YYYY-MM-DD)")]
    pub due_date: Option<String>,

    /// Standard ISO-4217 three-letter currency code (e.g. "USD", "EUR", "MXN", "CHF").
    #[schemars(description = "ISO-4217 currency code (e.g. USD, EUR)")]
    pub currency: String,

    /// Breakdown of individual invoice line items.
    #[schemars(description = "List of billed line items")]
    pub line_items: Vec<LineItem>,

    /// Net subtotal before taxes and discounts.
    #[schemars(description = "Net invoice subtotal before taxes and discounts")]
    pub subtotal_amount: f64,

    /// Total tax amount applied.
    #[schemars(description = "Total taxes applied")]
    pub tax_amount: f64,

    /// Optional discount or promotional deduction.
    #[schemars(description = "Discount deducted from subtotal")]
    pub discount_amount: Option<f64>,

    /// Final total amount due (`subtotal + tax - discount`).
    #[schemars(description = "Grand total amount due")]
    pub total_amount: f64,

    /// Destination banking and remittance coordinates.
    #[schemars(description = "Remittance payment instructions")]
    pub payment_info: Option<PaymentInfo>,
}

impl DocuValidate for InvoiceData {
    fn validate_integrity(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        const TOLERANCE: f64 = 0.05;

        // 1. Line items arithmetic check: quantity * unit_price == total
        for (i, item) in self.line_items.iter().enumerate() {
            let expected_total = item.quantity * item.unit_price;
            if (expected_total - item.total).abs() > TOLERANCE {
                errors.push(ValidationError {
                    field_path: format!("line_items[{}].total", i),
                    error_message: format!(
                        "Item '{}' total ({:.2}) does not match quantity ({}) * unit_price ({:.2}) = {:.2}",
                        item.description, item.total, item.quantity, item.unit_price, expected_total
                    ),
                });
            }
        }

        // 2. Sum of line items check: sum(line_items.total) == subtotal_amount
        if !self.line_items.is_empty() {
            let items_sum: f64 = self.line_items.iter().map(|item| item.total).sum();
            if (items_sum - self.subtotal_amount).abs() > TOLERANCE {
                errors.push(ValidationError {
                    field_path: "subtotal_amount".to_string(),
                    error_message: format!(
                        "Sum of line items ({:.2}) does not match subtotal_amount ({:.2})",
                        items_sum, self.subtotal_amount
                    ),
                });
            }
        }

        // 3. Grand total equation: subtotal + tax - discount == total_amount
        let discount = self.discount_amount.unwrap_or(0.0);
        let calculated_total = self.subtotal_amount + self.tax_amount - discount;
        if (calculated_total - self.total_amount).abs() > TOLERANCE {
            errors.push(ValidationError {
                field_path: "total_amount".to_string(),
                error_message: format!(
                    "Calculated total ({:.2} = subtotal {:.2} + tax {:.2} - discount {:.2}) does not match total_amount ({:.2})",
                    calculated_total, self.subtotal_amount, self.tax_amount, discount, self.total_amount
                ),
            });
        }

        // 4. Currency code check: ISO-4217 length check
        let currency = self.currency.trim();
        if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_uppercase()) {
            errors.push(ValidationError {
                field_path: "currency".to_string(),
                error_message: format!(
                    "Currency '{}' must be a valid 3-letter uppercase ISO-4217 code (e.g. USD, EUR)",
                    self.currency
                ),
            });
        }

        // 5. Vendor name non-empty check
        if self.vendor_name.trim().is_empty() {
            errors.push(ValidationError {
                field_path: "vendor_name".to_string(),
                error_message: "Vendor name must not be empty".to_string(),
            });
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(feature = "vision-codes")]
impl InvoiceData {
    /// Enriches payment details and recipient metadata from a decoded SEPA EPC QR code.
    pub fn apply_sepa_epc(&mut self, epc: &SepaEpcPayload) {
        let payment = self.payment_info.get_or_insert_with(PaymentInfo::default);
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
    pub fn apply_sat_cfdi(&mut self, cfdi: &MexicanSatCfdiPayload) {
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
    pub fn apply_swiss_qr(&mut self, swiss: &SwissQrPayload) {
        let payment = self.payment_info.get_or_insert_with(PaymentInfo::default);
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
