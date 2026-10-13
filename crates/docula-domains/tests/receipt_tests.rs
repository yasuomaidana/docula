use docula_domains::{docula_receipt, ReceiptData, ReceiptItem};
use docula_schema::DocuValidate;

docula_receipt! {
    pub struct CorporateExpenseReceipt {
        pub employee_id: String,
        pub project_code: String,
        pub reimbursable: bool,
    }
}

#[test]
fn test_receipt_math_valid_passes() {
    let receipt = ReceiptData {
        merchant_name: "Blue Bottle Coffee".to_string(),
        merchant_tax_id: Some("US-TAX-1029".to_string()),
        merchant_address: Some("315 Linden St, San Francisco, CA".to_string()),
        transaction_date: "2026-10-12".to_string(),
        transaction_time: Some("08:45:00".to_string()),
        currency: "USD".to_string(),
        subtotal_amount: Some(15.0),
        tax_amount: Some(1.35),
        tip_amount: Some(3.0),
        total_amount: 19.35, // 15 + 1.35 + 3.0 = 19.35
        payment_method: Some("Apple Pay".to_string()),
        items: vec![
            ReceiptItem::itemized("Pour Over Coffee", 2.0, 5.50), // 11.0
            ReceiptItem::new("Almond Croissant", 4.0),           // 4.0 -> sum = 15.0
        ],
    };

    assert!(receipt.validate_integrity().is_ok());
}

#[test]
fn test_receipt_totals_mismatch_fails() {
    let receipt = ReceiptData {
        merchant_name: "Bistro Paris".to_string(),
        merchant_tax_id: None,
        merchant_address: None,
        transaction_date: "2026-10-12".to_string(),
        transaction_time: None,
        currency: "EUR".to_string(),
        subtotal_amount: Some(50.0),
        tax_amount: Some(10.0),
        tip_amount: Some(5.0),
        total_amount: 70.0, // Expected 65.0!
        payment_method: None,
        items: vec![],
    };

    let res = receipt.validate_integrity();
    assert!(res.is_err());
    let errors = res.unwrap_err();
    assert!(errors.iter().any(|e| e.field_path == "total_amount"));
}

#[test]
fn test_receipt_items_sum_mismatch_fails() {
    let receipt = ReceiptData {
        merchant_name: "Tech Hardware".to_string(),
        merchant_tax_id: None,
        merchant_address: None,
        transaction_date: "2026-10-12".to_string(),
        transaction_time: None,
        currency: "USD".to_string(),
        subtotal_amount: Some(100.0),
        tax_amount: Some(10.0),
        tip_amount: None,
        total_amount: 110.0,
        payment_method: None,
        items: vec![
            ReceiptItem::new("USB Cable", 15.0), // Sum is 15.0, but subtotal is 100.0!
        ],
    };

    let res = receipt.validate_integrity();
    assert!(res.is_err());
    let errors = res.unwrap_err();
    assert!(errors.iter().any(|e| e.field_path == "subtotal_amount"));
}

#[test]
fn test_macro_receipt_custom_fields_and_schema() {
    let receipt = CorporateExpenseReceipt {
        merchant_name: "Enterprise Rent-A-Car".to_string(),
        merchant_tax_id: None,
        merchant_address: Some("SFO Airport".to_string()),
        transaction_date: "2026-10-10".to_string(),
        transaction_time: None,
        currency: "USD".to_string(),
        subtotal_amount: Some(250.0),
        tax_amount: Some(25.0),
        tip_amount: None,
        total_amount: 275.0,
        payment_method: Some("Corporate Amex".to_string()),
        items: vec![],
        employee_id: "EMP-4921".to_string(),
        project_code: "PRJ-DOCULA-Q4".to_string(),
        reimbursable: true,
    };

    assert!(receipt.validate_integrity().is_ok());

    let serialized = serde_json::to_string(&receipt).expect("serialize");
    let deserialized: CorporateExpenseReceipt =
        serde_json::from_str(&serialized).expect("deserialize");
    assert_eq!(receipt, deserialized);
    assert_eq!(deserialized.employee_id, "EMP-4921");
    assert_eq!(deserialized.project_code, "PRJ-DOCULA-Q4");

    let schema = schemars::schema_for!(CorporateExpenseReceipt);
    let schema_json = serde_json::to_string(&schema).expect("schema");
    assert!(schema_json.contains("employee_id"));
    assert!(schema_json.contains("project_code"));
    assert!(schema_json.contains("reimbursable"));
}
