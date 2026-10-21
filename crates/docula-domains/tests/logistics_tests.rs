use docula_domains::{BillOfLadingData, CargoItem, ShippingContainer, docula_bill_of_lading};
use docula_schema::DocuValidate;

docula_bill_of_lading! {
    pub struct CustomIntermodalBillOfLading {
        pub customs_entry_number: String,
        pub freight_payment_terms: String,
        pub destination_cfs: Option<String>,
    }
}

#[test]
fn test_bill_of_lading_valid_iso_checksum_passes() {
    let bol = BillOfLadingData {
        bol_number: "MAEU-9920194".to_string(),
        carrier_name: "Maersk Line".to_string(),
        shipper_name: "Yokohama Heavy Industries".to_string(),
        consignee_name: "Rotterdam Port Logistics".to_string(),
        vessel_name: Some("Maersk Mc-Kinney Moller".to_string()),
        voyage_number: Some("2604W".to_string()),
        port_of_loading: "Yokohama, Japan".to_string(),
        port_of_discharge: "Rotterdam, Netherlands".to_string(),
        containers: vec![ShippingContainer {
            container_number: "CSQU3054383".to_string(), // Valid ISO 6346!
            seal_number: Some("SL-99120".to_string()),
            container_type: Some("40HC".to_string()),
            weight_kg: Some(3800.0),
        }],
        cargo_items: vec![CargoItem {
            description: "Precision Industrial Robot Arms".to_string(),
            quantity: 12,
            package_type: Some("Crates".to_string()),
            gross_weight_kg: 18500.0,
            volume_cbm: Some(58.2),
            hazmat: false,
        }],
        total_weight_kg: 18500.0,
    };

    assert!(bol.validate_integrity().is_ok());
}

#[test]
fn test_bill_of_lading_corrupt_container_checksum_fails() {
    let bol = BillOfLadingData {
        bol_number: "MSC-10293".to_string(),
        carrier_name: "MSC".to_string(),
        shipper_name: "Shipper A".to_string(),
        consignee_name: "Consignee B".to_string(),
        vessel_name: None,
        voyage_number: None,
        port_of_loading: "Shanghai".to_string(),
        port_of_discharge: "Los Angeles".to_string(),
        containers: vec![
            ShippingContainer::new("CSQU3054387"), // Check digit should be 3, not 7!
        ],
        cargo_items: vec![],
        total_weight_kg: 0.0,
    };

    let res = bol.validate_integrity();
    assert!(res.is_err());
    let errors = res.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.field_path == "containers[0].container_number")
    );
}

#[test]
fn test_bill_of_lading_invalid_container_length_fails() {
    let bol = BillOfLadingData {
        bol_number: "CMA-551".to_string(),
        carrier_name: "CMA CGM".to_string(),
        shipper_name: "Shipper".to_string(),
        consignee_name: "Consignee".to_string(),
        vessel_name: None,
        voyage_number: None,
        port_of_loading: "Le Havre".to_string(),
        port_of_discharge: "New York".to_string(),
        containers: vec![
            ShippingContainer::new("CSQU305"), // Only 7 characters!
        ],
        cargo_items: vec![],
        total_weight_kg: 0.0,
    };

    let res = bol.validate_integrity();
    assert!(res.is_err());
    let errors = res.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.field_path == "containers[0].container_number")
    );
}

#[test]
fn test_bill_of_lading_cargo_weight_sum_mismatch() {
    let bol = BillOfLadingData {
        bol_number: "HAPAG-881".to_string(),
        carrier_name: "Hapag-Lloyd".to_string(),
        shipper_name: "Shipper".to_string(),
        consignee_name: "Consignee".to_string(),
        vessel_name: None,
        voyage_number: None,
        port_of_loading: "Hamburg".to_string(),
        port_of_discharge: "Singapore".to_string(),
        containers: vec![ShippingContainer::new("CSQU3054383")],
        cargo_items: vec![CargoItem {
            description: "Auto Parts".to_string(),
            quantity: 100,
            package_type: None,
            gross_weight_kg: 5000.0,
            volume_cbm: None,
            hazmat: false,
        }],
        total_weight_kg: 7000.0, // Stated 7000 vs item 5000!
    };

    let res = bol.validate_integrity();
    assert!(res.is_err());
    let errors = res.unwrap_err();
    assert!(errors.iter().any(|e| e.field_path == "total_weight_kg"));
}

#[test]
fn test_macro_bill_of_lading_custom_fields_and_schema() {
    let bol = CustomIntermodalBillOfLading {
        bol_number: "ONE-7721".to_string(),
        carrier_name: "Ocean Network Express".to_string(),
        shipper_name: "Tokyo Cargo".to_string(),
        consignee_name: "Oakland Terminal".to_string(),
        vessel_name: Some("ONE Apus".to_string()),
        voyage_number: Some("012E".to_string()),
        port_of_loading: "Tokyo".to_string(),
        port_of_discharge: "Oakland".to_string(),
        containers: vec![ShippingContainer::new("CSQU3054383")],
        cargo_items: vec![],
        total_weight_kg: 0.0,
        customs_entry_number: "US-CBP-9920184".to_string(),
        freight_payment_terms: "PREPAID".to_string(),
        destination_cfs: Some("CFS-OAKLAND-NORTH".to_string()),
    };

    assert!(bol.validate_integrity().is_ok());

    let serialized = serde_json::to_string(&bol).expect("serialize");
    let deserialized: CustomIntermodalBillOfLading =
        serde_json::from_str(&serialized).expect("deserialize");
    assert_eq!(bol, deserialized);
    assert_eq!(deserialized.customs_entry_number, "US-CBP-9920184");
    assert_eq!(deserialized.freight_payment_terms, "PREPAID");

    let schema = schemars::schema_for!(CustomIntermodalBillOfLading);
    let schema_json = serde_json::to_string(&schema).expect("schema");
    assert!(schema_json.contains("customs_entry_number"));
    assert!(schema_json.contains("freight_payment_terms"));
    assert!(schema_json.contains("destination_cfs"));
}
