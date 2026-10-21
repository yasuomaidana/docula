//! Bank Statement and Financial Reconciliation domain schema, macros, and validation.
//!
//! Provides the [`docula_bank_statement!`] extensible macro, composable [`BankTransaction`],
//! [`TransactionType`], and the standard [`BankStatementData`] preset.
//! Implements running balance ledger continuity checks via [`docula_schema::DocuValidate`].

pub use docula_schema::DocuValidate;
use docula_schema::ValidationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Classification of a bank transaction as an inflow (Credit) or outflow (Debit).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    /// Inflow of funds into the account.
    #[default]
    Credit,
    /// Outflow of funds from the account.
    Debit,
}

/// An individual line item transaction on a bank or financial statement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BankTransaction {
    /// Transaction booking or posting date (YYYY-MM-DD).
    #[schemars(description = "Transaction posting date (YYYY-MM-DD)")]
    pub date: String,

    /// Payee, remitter, or narrative description of the transaction.
    #[schemars(description = "Description or counterparty narrative")]
    pub description: String,

    /// Absolute monetary amount of the transaction.
    #[schemars(description = "Absolute monetary transaction amount")]
    pub amount: f64,

    /// Type of transaction (Credit / Debit).
    #[schemars(description = "Transaction direction: credit or debit")]
    pub transaction_type: TransactionType,

    /// Optional reference, check number, or transaction identifier.
    #[schemars(description = "Transaction reference or check number")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,

    /// Optional running balance after this transaction was applied.
    #[schemars(description = "Running balance after transaction execution")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance_after: Option<f64>,
}

impl BankTransaction {
    /// Constructs a new credit transaction.
    #[must_use]
    pub fn credit(date: impl Into<String>, description: impl Into<String>, amount: f64) -> Self {
        Self {
            date: date.into(),
            description: description.into(),
            amount: (amount * 100.0).round() / 100.0,
            transaction_type: TransactionType::Credit,
            reference: None,
            balance_after: None,
        }
    }

    /// Constructs a new debit transaction.
    #[must_use]
    pub fn debit(date: impl Into<String>, description: impl Into<String>, amount: f64) -> Self {
        Self {
            date: date.into(),
            description: description.into(),
            amount: (amount * 100.0).round() / 100.0,
            transaction_type: TransactionType::Debit,
            reference: None,
            balance_after: None,
        }
    }
}

// =========================================================================
// Modular Complexity-Bounded Integrity Validators (CC <= 6, Cognitive <= 6)
// =========================================================================

/// Validates ledger continuity: opening_balance + sum(credits) - sum(debits) == closing_balance.
pub fn validate_ledger_continuity(
    opening: f64,
    closing: f64,
    transactions: &[BankTransaction],
    tolerance: f64,
) -> Result<(), ValidationError> {
    let mut net_flow = 0.0;
    for tx in transactions {
        match tx.transaction_type {
            TransactionType::Credit => net_flow += tx.amount,
            TransactionType::Debit => net_flow -= tx.amount,
        }
    }

    let calculated_closing = opening + net_flow;
    if (calculated_closing - closing).abs() > tolerance {
        Err(ValidationError {
            field_path: "closing_balance".to_string(),
            error_message: format!(
                "Calculated closing balance ({:.2} = opening {:.2} + net flow {:.2}) does not match closing_balance ({:.2})",
                calculated_closing, opening, net_flow, closing
            ),
        })
    } else {
        Ok(())
    }
}

/// Validates total credits and debits if reported explicitly.
pub fn validate_flow_totals(
    transactions: &[BankTransaction],
    total_credits: Option<f64>,
    total_debits: Option<f64>,
    tolerance: f64,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let sum_credits: f64 = transactions
        .iter()
        .filter(|t| t.transaction_type == TransactionType::Credit)
        .map(|t| t.amount)
        .sum();

    let sum_debits: f64 = transactions
        .iter()
        .filter(|t| t.transaction_type == TransactionType::Debit)
        .map(|t| t.amount)
        .sum();

    if let Some(expected_credits) = total_credits
        && (sum_credits - expected_credits).abs() > tolerance
    {
        errors.push(ValidationError {
            field_path: "total_credits".to_string(),
            error_message: format!(
                "Sum of credit transactions ({:.2}) does not match stated total_credits ({:.2})",
                sum_credits, expected_credits
            ),
        });
    }

    if let Some(expected_debits) = total_debits
        && (sum_debits - expected_debits).abs() > tolerance
    {
        errors.push(ValidationError {
            field_path: "total_debits".to_string(),
            error_message: format!(
                "Sum of debit transactions ({:.2}) does not match stated total_debits ({:.2})",
                sum_debits, expected_debits
            ),
        });
    }

    errors
}

/// Validates sequential running balance for transactions with `balance_after`.
pub fn validate_running_balances(
    opening: f64,
    transactions: &[BankTransaction],
    tolerance: f64,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut running = opening;

    for (i, tx) in transactions.iter().enumerate() {
        match tx.transaction_type {
            TransactionType::Credit => running += tx.amount,
            TransactionType::Debit => running -= tx.amount,
        }

        if let Some(stated_balance) = tx.balance_after
            && (running - stated_balance).abs() > tolerance
        {
            errors.push(ValidationError {
                field_path: format!("transactions[{}].balance_after", i),
                error_message: format!(
                    "Transaction {} calculated running balance ({:.2}) does not match stated balance_after ({:.2})",
                    i, running, stated_balance
                ),
            });
        }
    }

    errors
}

// =========================================================================
// Macro Generator: docula_bank_statement!
// =========================================================================

/// Macro synthesizing customizable bank statement schemas with standard financial fields,
/// custom user reconciliation tags, and automated ledger continuity validation.
#[macro_export]
macro_rules! docula_bank_statement {
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
            #[schemars(description = "Legal issuing bank or financial institution name")]
            pub bank_name: String,

            #[schemars(description = "Account number or IBAN")]
            pub account_number: String,

            #[schemars(description = "Statement period start date (YYYY-MM-DD)")]
            pub statement_period_start: String,

            #[schemars(description = "Statement period end date (YYYY-MM-DD)")]
            pub statement_period_end: String,

            #[schemars(description = "ISO-4217 3-letter currency code (e.g. USD, EUR)")]
            pub currency: String,

            #[schemars(description = "Starting account balance at beginning of period")]
            pub opening_balance: f64,

            #[schemars(description = "Ending account balance at close of period")]
            pub closing_balance: f64,

            #[schemars(description = "List of posted transactions within statement period")]
            pub transactions: Vec<$crate::bank::BankTransaction>,

            #[schemars(description = "Total credit inflow amount")]
            pub total_credits: Option<f64>,

            #[schemars(description = "Total debit outflow amount")]
            pub total_debits: Option<f64>,

            $(
                $(#[$field_meta])*
                $field_vis $field_name: $field_ty,
            )*
        }

        impl $crate::docula_schema::DocuValidate for $name {
            fn validate_integrity(&self) -> Result<(), Vec<$crate::docula_schema::ValidationError>> {
                let mut errors = Vec::new();
                const TOLERANCE: f64 = 0.05;

                if let Err(e) = $crate::bank::validate_ledger_continuity(
                    self.opening_balance,
                    self.closing_balance,
                    &self.transactions,
                    TOLERANCE,
                ) {
                    errors.push(e);
                }

                errors.extend($crate::bank::validate_flow_totals(
                    &self.transactions,
                    self.total_credits,
                    self.total_debits,
                    TOLERANCE,
                ));

                errors.extend($crate::bank::validate_running_balances(
                    self.opening_balance,
                    &self.transactions,
                    TOLERANCE,
                ));

                if let Err(e) = $crate::invoice::validate_iso_currency(&self.currency) {
                    errors.push(e);
                }

                if self.bank_name.trim().is_empty() {
                    errors.push($crate::docula_schema::ValidationError {
                        field_path: "bank_name".to_string(),
                        error_message: "Bank name must not be empty".to_string(),
                    });
                }

                if self.account_number.trim().is_empty() {
                    errors.push($crate::docula_schema::ValidationError {
                        field_path: "account_number".to_string(),
                        error_message: "Account number must not be empty".to_string(),
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

// Preset standard BankStatementData instantiation
docula_bank_statement! {
    /// Standardized bank statement schema with ledger continuity and balance reconciliation.
    pub struct BankStatementData {}
}
