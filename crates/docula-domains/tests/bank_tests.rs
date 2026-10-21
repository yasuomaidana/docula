use docula_domains::{BankStatementData, BankTransaction, TransactionType, docula_bank_statement};
use docula_schema::DocuValidate;

docula_bank_statement! {
    pub struct CustomReconciledBankStatement {
        pub gl_account_id: String,
        pub internal_audit_notes: Option<String>,
        pub reconciliation_status: String,
    }
}

#[test]
fn test_bank_statement_balanced_ledger_passes() {
    let statement = BankStatementData {
        bank_name: "JPMorgan Chase".to_string(),
        account_number: "CHAS-99210-441".to_string(),
        statement_period_start: "2026-09-01".to_string(),
        statement_period_end: "2026-09-30".to_string(),
        currency: "USD".to_string(),
        opening_balance: 10000.0,
        closing_balance: 11500.0,
        transactions: vec![
            BankTransaction {
                date: "2026-09-05".to_string(),
                description: "Client Wire Deposit".to_string(),
                amount: 3000.0,
                transaction_type: TransactionType::Credit,
                reference: Some("WIRE-881".to_string()),
                balance_after: Some(13000.0),
            },
            BankTransaction {
                date: "2026-09-12".to_string(),
                description: "Cloud Hosting Provider".to_string(),
                amount: 1500.0,
                transaction_type: TransactionType::Debit,
                reference: Some("ACH-220".to_string()),
                balance_after: Some(11500.0),
            },
        ],
        total_credits: Some(3000.0),
        total_debits: Some(1500.0),
    };

    assert!(statement.validate_integrity().is_ok());
}

#[test]
fn test_bank_statement_discrepancy_detected() {
    let statement = BankStatementData {
        bank_name: "Barclays".to_string(),
        account_number: "BARC-12345".to_string(),
        statement_period_start: "2026-09-01".to_string(),
        statement_period_end: "2026-09-30".to_string(),
        currency: "GBP".to_string(),
        opening_balance: 5000.0,
        closing_balance: 6000.0, // Expected 5000 + 2000 - 500 = 6500!
        transactions: vec![
            BankTransaction::credit("2026-09-02", "Transfer In", 2000.0),
            BankTransaction::debit("2026-09-10", "Utility Bill", 500.0),
        ],
        total_credits: None,
        total_debits: None,
    };

    let res = statement.validate_integrity();
    assert!(res.is_err());
    let errors = res.unwrap_err();
    assert!(errors.iter().any(|e| e.field_path == "closing_balance"));
}

#[test]
fn test_bank_statement_flow_totals_mismatch() {
    let statement = BankStatementData {
        bank_name: "Deutsche Bank".to_string(),
        account_number: "DE89370400440532013000".to_string(),
        statement_period_start: "2026-09-01".to_string(),
        statement_period_end: "2026-09-30".to_string(),
        currency: "EUR".to_string(),
        opening_balance: 1000.0,
        closing_balance: 1500.0,
        transactions: vec![BankTransaction::credit("2026-09-02", "Payroll", 500.0)],
        total_credits: Some(800.0), // Mismatch!
        total_debits: None,
    };

    let res = statement.validate_integrity();
    assert!(res.is_err());
    let errors = res.unwrap_err();
    assert!(errors.iter().any(|e| e.field_path == "total_credits"));
}

#[test]
fn test_bank_statement_running_balance_mismatch() {
    let mut tx = BankTransaction::credit("2026-09-02", "Deposit", 500.0);
    tx.balance_after = Some(2000.0); // 1000 + 500 = 1500, not 2000!

    let statement = BankStatementData {
        bank_name: "Chase".to_string(),
        account_number: "123".to_string(),
        statement_period_start: "2026-09-01".to_string(),
        statement_period_end: "2026-09-30".to_string(),
        currency: "USD".to_string(),
        opening_balance: 1000.0,
        closing_balance: 1500.0,
        transactions: vec![tx],
        total_credits: None,
        total_debits: None,
    };

    let res = statement.validate_integrity();
    assert!(res.is_err());
    let errors = res.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.field_path == "transactions[0].balance_after")
    );
}

#[test]
fn test_macro_bank_statement_custom_fields_and_schema() {
    let stmt = CustomReconciledBankStatement {
        bank_name: "BNP Paribas".to_string(),
        account_number: "FR763000600001".to_string(),
        statement_period_start: "2026-09-01".to_string(),
        statement_period_end: "2026-09-30".to_string(),
        currency: "EUR".to_string(),
        opening_balance: 20000.0,
        closing_balance: 22000.0,
        transactions: vec![BankTransaction::credit(
            "2026-09-05",
            "Customer Inbound",
            2000.0,
        )],
        total_credits: Some(2000.0),
        total_debits: Some(0.0),
        gl_account_id: "GL-1001-CASH".to_string(),
        internal_audit_notes: Some("Reviewed by auditor".to_string()),
        reconciliation_status: "RECONCILED".to_string(),
    };

    assert!(stmt.validate_integrity().is_ok());

    let serialized = serde_json::to_string(&stmt).expect("serialize");
    let deserialized: CustomReconciledBankStatement =
        serde_json::from_str(&serialized).expect("deserialize");
    assert_eq!(stmt, deserialized);
    assert_eq!(deserialized.gl_account_id, "GL-1001-CASH");

    let schema = schemars::schema_for!(CustomReconciledBankStatement);
    let schema_json = serde_json::to_string(&schema).expect("schema");
    assert!(schema_json.contains("gl_account_id"));
    assert!(schema_json.contains("internal_audit_notes"));
}
