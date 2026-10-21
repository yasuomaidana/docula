use docula_domains::{InvoiceData, LineItem, PaymentInfo};
use docula_schema::DocuValidate;

#[test]
fn test_invoice_valid_arithmetic_passes() {
    let invoice = InvoiceData {
        vendor_name: "Acme Industrial Supplies".to_string(),
        vendor_tax_id: Some("US-EIN-99238104".to_string()),
        invoice_number: Some("INV-2026-001".to_string()),
        issue_date: "2026-10-12".to_string(),
        due_date: Some("2026-11-12".to_string()),
        currency: "USD".to_string(),
        line_items: vec![
            LineItem::new("Industrial Widget A", 2.0, 50.0),
            LineItem::new("Precision Bolt Set", 10.0, 5.0),
        ],
        subtotal_amount: 150.0,
        tax_amount: 15.0,
        discount_amount: Some(10.0),
        total_amount: 155.0,
        payment_info: Some(PaymentInfo {
            iban: Some("US123456789".to_string()),
            bic: Some("CHASUS33".to_string()),
            beneficiary_name: Some("Acme Inc".to_string()),
            reference: Some("INV-2026-001".to_string()),
        }),
    };

    assert!(invoice.validate_integrity().is_ok());
}

#[test]
fn test_invoice_arithmetic_mismatch_fails() {
    let invoice = InvoiceData {
        vendor_name: "Acme Corp".to_string(),
        vendor_tax_id: None,
        invoice_number: None,
        issue_date: "2026-10-12".to_string(),
        due_date: None,
        currency: "USD".to_string(),
        line_items: vec![LineItem::new("Widget", 1.0, 100.0)],
        subtotal_amount: 100.0,
        tax_amount: 10.0,
        discount_amount: None,
        total_amount: 120.0, // Should be 110.0!
        payment_info: None,
    };

    let res = invoice.validate_integrity();
    assert!(res.is_err());
    let errors = res.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field_path, "total_amount");
}

#[test]
fn test_invoice_line_item_math_fails() {
    let mut item = LineItem::new("Corrupt Line", 5.0, 20.0);
    item.total = 80.0; // Expected 100.0

    let invoice = InvoiceData {
        vendor_name: "Acme Corp".to_string(),
        vendor_tax_id: None,
        invoice_number: None,
        issue_date: "2026-10-12".to_string(),
        due_date: None,
        currency: "USD".to_string(),
        line_items: vec![item],
        subtotal_amount: 80.0,
        tax_amount: 0.0,
        discount_amount: None,
        total_amount: 80.0,
        payment_info: None,
    };

    let res = invoice.validate_integrity();
    assert!(res.is_err());
    let errors = res.unwrap_err();
    assert!(errors.iter().any(|e| e.field_path == "line_items[0].total"));
}

#[test]
fn test_invoice_currency_validation() {
    let invoice = InvoiceData {
        vendor_name: "Acme Corp".to_string(),
        vendor_tax_id: None,
        invoice_number: None,
        issue_date: "2026-10-12".to_string(),
        due_date: None,
        currency: "usd".to_string(), // Lowercase invalid!
        line_items: vec![],
        subtotal_amount: 0.0,
        tax_amount: 0.0,
        discount_amount: None,
        total_amount: 0.0,
        payment_info: None,
    };

    let res = invoice.validate_integrity();
    assert!(res.is_err());
    let errors = res.unwrap_err();
    assert!(errors.iter().any(|e| e.field_path == "currency"));
}

#[test]
fn test_invoice_vendor_name_empty() {
    let invoice = InvoiceData {
        vendor_name: "   ".to_string(),
        vendor_tax_id: None,
        invoice_number: None,
        issue_date: "2026-10-12".to_string(),
        due_date: None,
        currency: "EUR".to_string(),
        line_items: vec![],
        subtotal_amount: 0.0,
        tax_amount: 0.0,
        discount_amount: None,
        total_amount: 0.0,
        payment_info: None,
    };

    let res = invoice.validate_integrity();
    assert!(res.is_err());
    let errors = res.unwrap_err();
    assert!(errors.iter().any(|e| e.field_path == "vendor_name"));
}

#[cfg(feature = "vision-codes")]
#[test]
fn test_invoice_sepa_epc_adapter() {
    use docula_vision_codes::types::SepaEpcPayload;

    let mut invoice = InvoiceData {
        vendor_name: "Euro Widgets GmbH".to_string(),
        vendor_tax_id: None,
        invoice_number: Some("INV-990".to_string()),
        issue_date: "2026-10-12".to_string(),
        due_date: None,
        currency: "".to_string(),
        line_items: vec![LineItem::new("Item A", 1.0, 450.75)],
        subtotal_amount: 450.75,
        tax_amount: 0.0,
        discount_amount: None,
        total_amount: 0.0,
        payment_info: None,
    };

    let epc = SepaEpcPayload {
        service_tag: "BCD".to_string(),
        version: "001".to_string(),
        character_set: "1".to_string(),
        identification: "SCT".to_string(),
        bic: "TESTBIC".to_string(),
        beneficiary_name: "Euro Widgets GmbH".to_string(),
        iban: "ES1234567890123456789012".to_string(),
        amount: Some(450.75),
        currency: "EUR".to_string(),
        purpose: None,
        remittance_reference: Some("RF18539007547034".to_string()),
        remittance_text: Some("Invoice 990".to_string()),
    };

    invoice.apply_sepa_epc(&epc);
    assert_eq!(invoice.currency, "EUR");
    assert_eq!(invoice.total_amount, 450.75);
    let payment = invoice.payment_info.as_ref().unwrap();
    assert_eq!(payment.iban, Some("ES1234567890123456789012".to_string()));
    assert_eq!(payment.bic, Some("TESTBIC".to_string()));
    assert_eq!(payment.reference, Some("Invoice 990".to_string()));
    assert!(invoice.validate_integrity().is_ok());
}

#[cfg(feature = "vision-codes")]
#[test]
fn test_invoice_sat_cfdi_adapter() {
    use docula_vision_codes::types::MexicanSatCfdiPayload;

    let mut invoice = InvoiceData {
        vendor_name: "Empresa Mexicana SA de CV".to_string(),
        vendor_tax_id: None,
        invoice_number: None,
        issue_date: "2026-10-12".to_string(),
        due_date: None,
        currency: "USD".to_string(),
        line_items: vec![],
        subtotal_amount: 1000.0,
        tax_amount: 160.0,
        discount_amount: None,
        total_amount: 0.0,
        payment_info: None,
    };

    let cfdi = MexicanSatCfdiPayload {
        uuid: "12345678-ABCD-1234-ABCD-1234567890AB".to_string(),
        rfc_issuer: "AAA010101AAA".to_string(),
        rfc_receiver: "BBB010101BBB".to_string(),
        total: 1160.0,
        fe: "abc12345".to_string(),
    };

    invoice.apply_sat_cfdi(&cfdi);
    assert_eq!(invoice.currency, "MXN");
    assert_eq!(invoice.total_amount, 1160.0);
    assert_eq!(invoice.vendor_tax_id, Some("AAA010101AAA".to_string()));
    assert_eq!(
        invoice.invoice_number,
        Some("12345678-ABCD-1234-ABCD-1234567890AB".to_string())
    );
    assert!(invoice.validate_integrity().is_ok());
}

#[cfg(feature = "vision-codes")]
#[test]
fn test_invoice_swiss_qr_adapter() {
    use docula_vision_codes::types::SwissQrPayload;

    let mut invoice = InvoiceData {
        vendor_name: "Schweizer AG".to_string(),
        vendor_tax_id: None,
        invoice_number: None,
        issue_date: "2026-10-12".to_string(),
        due_date: None,
        currency: "".to_string(),
        line_items: vec![],
        subtotal_amount: 320.50,
        tax_amount: 0.0,
        discount_amount: None,
        total_amount: 0.0,
        payment_info: None,
    };

    let swiss = SwissQrPayload {
        version: "0200".to_string(),
        currency: "CHF".to_string(),
        amount: Some(320.50),
        iban: "CH9300762011623852957".to_string(),
        creditor_name: "Schweizer AG".to_string(),
        creditor_address: "Bahnhofstrasse 10, Zurich".to_string(),
        reference: Some("210000000003139471530009017".to_string()),
        message: Some("Rechnung Oktober".to_string()),
    };

    invoice.apply_swiss_qr(&swiss);
    assert_eq!(invoice.currency, "CHF");
    assert_eq!(invoice.total_amount, 320.50);
    let payment = invoice.payment_info.as_ref().unwrap();
    assert_eq!(payment.iban, Some("CH9300762011623852957".to_string()));
    assert_eq!(payment.beneficiary_name, Some("Schweizer AG".to_string()));
    assert!(invoice.validate_integrity().is_ok());
}

#[test]
fn test_invoice_serde_and_schema_reflection() {
    let schema = schemars::schema_for!(InvoiceData);
    let schema_json = serde_json::to_string(&schema).expect("valid schema JSON");
    assert!(schema_json.contains("vendor_name"));
    assert!(schema_json.contains("line_items"));
    assert!(schema_json.contains("total_amount"));

    let invoice = InvoiceData {
        vendor_name: "Test Vendor".to_string(),
        vendor_tax_id: None,
        invoice_number: Some("INV-1".to_string()),
        issue_date: "2026-10-12".to_string(),
        due_date: None,
        currency: "USD".to_string(),
        line_items: vec![LineItem::new("Item 1", 1.0, 10.0)],
        subtotal_amount: 10.0,
        tax_amount: 0.0,
        discount_amount: None,
        total_amount: 10.0,
        payment_info: None,
    };

    let serialized = serde_json::to_string(&invoice).expect("serialization succeeds");
    let deserialized: InvoiceData =
        serde_json::from_str(&serialized).expect("deserialization succeeds");
    assert_eq!(invoice, deserialized);
}
