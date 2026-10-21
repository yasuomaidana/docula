use docula_domains::{LineItem, docula_invoice};
use docula_schema::DocuValidate;

// Define a custom invoice with line-item and invoice discounts, plus custom enterprise fields
docula_invoice! {
    pub struct CustomEnterpriseInvoice {
        discount_policy = both,
        pub cost_center: Option<String>,
        pub purchase_order: Option<String>,
        pub internal_approval: bool,
    }
}

// Define an invoice that strictly uses line-item discounts only
docula_invoice! {
    pub struct LineItemDiscountOnlyInvoice {
        discount_policy = line_item,
        pub department: String,
    }
}

// Define an invoice with zero discounts
docula_invoice! {
    pub struct SimpleNoDiscountInvoice {
        discount_policy = none,
    }
}

#[test]
fn test_macro_invoice_custom_fields_and_schema_reflection() {
    let invoice = CustomEnterpriseInvoice {
        vendor_name: "Sovereign AI Labs".to_string(),
        vendor_tax_id: Some("TAX-998811".to_string()),
        invoice_number: Some("INV-2026-999".to_string()),
        issue_date: "2026-10-12".to_string(),
        due_date: Some("2026-11-12".to_string()),
        currency: "USD".to_string(),
        line_items: vec![
            LineItem::with_discount("GPU Cluster Hour", 10.0, 50.0, 50.0), // 10*50 - 50 = 450
            LineItem::new("Storage TB", 2.0, 25.0),                        // 2*25 = 50
        ],
        subtotal_amount: 500.0,
        tax_amount: 50.0,
        discount_amount: Some(20.0), // Invoice level discount
        total_amount: 530.0,         // 500 + 50 - 20 = 530
        payment_info: None,
        cost_center: Some("CC-ENGINEERING-402".to_string()),
        purchase_order: Some("PO-88219".to_string()),
        internal_approval: true,
    };

    // 1. Math passes
    assert!(invoice.validate_integrity().is_ok());

    // 2. Serde roundtrip preserves custom fields
    let serialized = serde_json::to_string(&invoice).expect("serialization succeeds");
    let deserialized: CustomEnterpriseInvoice =
        serde_json::from_str(&serialized).expect("deserialization succeeds");
    assert_eq!(invoice, deserialized);
    assert_eq!(
        deserialized.cost_center,
        Some("CC-ENGINEERING-402".to_string())
    );
    assert_eq!(deserialized.purchase_order, Some("PO-88219".to_string()));
    assert!(deserialized.internal_approval);

    // 3. JsonSchema contains custom fields for DFA grammar compilation
    let schema = schemars::schema_for!(CustomEnterpriseInvoice);
    let schema_json = serde_json::to_string(&schema).expect("schema serialization succeeds");
    assert!(schema_json.contains("cost_center"));
    assert!(schema_json.contains("purchase_order"));
    assert!(schema_json.contains("internal_approval"));
    assert!(schema_json.contains("discount_amount"));
}

#[test]
fn test_macro_invoice_line_item_discount_arithmetic() {
    let invoice = LineItemDiscountOnlyInvoice {
        vendor_name: "Hardware Depot".to_string(),
        vendor_tax_id: None,
        invoice_number: Some("HW-101".to_string()),
        issue_date: "2026-10-12".to_string(),
        due_date: None,
        currency: "EUR".to_string(),
        line_items: vec![
            LineItem::with_discount("Rack Server", 2.0, 1000.0, 200.0), // 2*1000 - 200 = 1800
        ],
        subtotal_amount: 1800.0,
        tax_amount: 360.0,
        total_amount: 2160.0, // 1800 + 360 = 2160 (no invoice level discount)
        payment_info: None,
        department: "DevOps".to_string(),
    };

    assert!(invoice.validate_integrity().is_ok());
}

#[test]
fn test_macro_invoice_line_item_discount_mismatch_detected() {
    // If the line total does not account for the line discount:
    let mut corrupt_item = LineItem::with_discount("Rack Server", 2.0, 1000.0, 200.0);
    corrupt_item.total = 2000.0; // Forgot to deduct 200.0 discount!

    let invoice = LineItemDiscountOnlyInvoice {
        vendor_name: "Hardware Depot".to_string(),
        vendor_tax_id: None,
        invoice_number: None,
        issue_date: "2026-10-12".to_string(),
        due_date: None,
        currency: "EUR".to_string(),
        line_items: vec![corrupt_item],
        subtotal_amount: 2000.0,
        tax_amount: 400.0,
        total_amount: 2400.0,
        payment_info: None,
        department: "DevOps".to_string(),
    };

    let res = invoice.validate_integrity();
    assert!(res.is_err());
    let errors = res.unwrap_err();
    assert!(errors.iter().any(|e| e.field_path == "line_items[0].total"));
}

#[test]
fn test_macro_invoice_no_discount_policy() {
    let invoice = SimpleNoDiscountInvoice {
        vendor_name: "Office Express".to_string(),
        vendor_tax_id: None,
        invoice_number: None,
        issue_date: "2026-10-12".to_string(),
        due_date: None,
        currency: "USD".to_string(),
        line_items: vec![LineItem::new("Pens", 10.0, 2.0)],
        subtotal_amount: 20.0,
        tax_amount: 2.0,
        total_amount: 22.0, // 20 + 2 = 22
        payment_info: None,
    };

    assert!(invoice.validate_integrity().is_ok());
}

#[cfg(feature = "vision-codes")]
#[test]
fn test_macro_invoice_optical_barcode_adapters() {
    use docula_vision_codes::types::SepaEpcPayload;

    let mut invoice = CustomEnterpriseInvoice {
        vendor_name: "Global Tech".to_string(),
        vendor_tax_id: None,
        invoice_number: None,
        issue_date: "2026-10-12".to_string(),
        due_date: None,
        currency: "".to_string(),
        line_items: vec![LineItem::new("Consulting", 1.0, 1000.0)],
        subtotal_amount: 1000.0,
        tax_amount: 0.0,
        discount_amount: None,
        total_amount: 0.0,
        payment_info: None,
        cost_center: None,
        purchase_order: None,
        internal_approval: false,
    };

    let epc = SepaEpcPayload {
        service_tag: "BCD".to_string(),
        version: "001".to_string(),
        character_set: "1".to_string(),
        identification: "SCT".to_string(),
        bic: "BNPAFRPP".to_string(),
        beneficiary_name: "Global Tech".to_string(),
        iban: "FR7630006000011234567890189".to_string(),
        amount: Some(1000.0),
        currency: "EUR".to_string(),
        purpose: None,
        remittance_reference: None,
        remittance_text: Some("Consulting Oct".to_string()),
    };

    invoice.apply_sepa_epc(&epc);
    assert_eq!(invoice.currency, "EUR");
    assert_eq!(invoice.total_amount, 1000.0);
    let payment = invoice.payment_info.as_ref().unwrap();
    assert_eq!(
        payment.iban,
        Some("FR7630006000011234567890189".to_string())
    );
    assert_eq!(payment.bic, Some("BNPAFRPP".to_string()));
    assert!(invoice.validate_integrity().is_ok());
}
