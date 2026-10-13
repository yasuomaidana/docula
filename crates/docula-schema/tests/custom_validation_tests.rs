use docula_schema::{DocuValidate, ValidationOutcome, docula_model};

// 1. Bill struct where total is strictly: sum(items) - discount
docula_model! {
    pub struct BillMinusDiscount {
        pub items_total: f64,
        pub discount: f64,
        pub total: f64,
    }
    validate(self, v) {
        let expected = self.items_total - self.discount;
        v.assert_approx_eq(
            self.total,
            expected,
            0.05,
            "total",
            "Total must equal items_total - discount",
        );
    }
}

// 2. Bill struct where total is strictly: sum(items) without discount
docula_model! {
    pub struct BillItemsOnly {
        pub items_total: f64,
        pub total: f64,
    }
    validate(self, v) {
        v.assert_approx_eq(
            self.total,
            self.items_total,
            0.05,
            "total",
            "Total must equal items_total exactly",
        );
    }
}

// 3. Bill struct with fallback assertion: total could be (items - discount), OR (items - tax), OR items
docula_model! {
    pub struct BillWithFallback {
        pub items_total: f64,
        pub discount: Option<f64>,
        pub tax: Option<f64>,
        pub total: f64,
    }
    validate(self, v) {
        let disc = self.discount.unwrap_or(0.0);
        let tx = self.tax.unwrap_or(0.0);

        let candidate_minus_discount = (self.total - (self.items_total - disc)).abs() <= 0.05;
        let candidate_minus_tax = (self.total - (self.items_total - tx)).abs() <= 0.05;
        let candidate_exact_sum = (self.total - self.items_total).abs() <= 0.05;

        v.assert_fallback(
            "total",
            &[
                ("items_minus_discount", candidate_minus_discount),
                ("items_minus_tax", candidate_minus_tax),
                ("exact_items_sum", candidate_exact_sum),
            ],
            "Total failed all acceptable accounting balance formulas",
        );
    }
}

// 4. Person struct validating that birthdate year is between 1920 and 2026
docula_model! {
    pub struct CustomerProfile {
        pub full_name: String,
        pub birthdate: String,
        pub account_tier: Option<String>,
    }
    validate(self, v) {
        v.assert_non_empty("full_name", &self.full_name, "Full name");
        v.assert_year_between("birthdate", &self.birthdate, 1920, 2026);
    }
}

#[test]
fn test_bill_minus_discount_assertion() {
    let valid = BillMinusDiscount {
        items_total: 100.0,
        discount: 15.0,
        total: 85.0,
    };
    assert!(valid.validate_integrity().is_ok());

    let invalid = BillMinusDiscount {
        items_total: 100.0,
        discount: 15.0,
        total: 100.0, // Should be 85.0!
    };
    let errs = invalid.validate_integrity().unwrap_err();
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].field_path, "total");
}

#[test]
fn test_bill_items_only_assertion() {
    let valid = BillItemsOnly {
        items_total: 250.0,
        total: 250.0,
    };
    assert!(valid.validate_integrity().is_ok());

    let invalid = BillItemsOnly {
        items_total: 250.0,
        total: 220.0,
    };
    assert!(invalid.validate_integrity().is_err());
}

#[test]
fn test_bill_fallback_assertion_logic() {
    // Branch 1: matches items - discount
    let bill_a = BillWithFallback {
        items_total: 100.0,
        discount: Some(10.0),
        tax: Some(5.0),
        total: 90.0, // 100 - 10
    };
    assert!(bill_a.validate_integrity().is_ok());

    // Branch 2: matches items - tax
    let bill_b = BillWithFallback {
        items_total: 100.0,
        discount: Some(10.0),
        tax: Some(5.0),
        total: 95.0, // 100 - 5
    };
    assert!(bill_b.validate_integrity().is_ok());

    // Branch 3: matches exact items_total
    let bill_c = BillWithFallback {
        items_total: 100.0,
        discount: None,
        tax: None,
        total: 100.0,
    };
    assert!(bill_c.validate_integrity().is_ok());

    // Branch 4: matches NONE of the fallbacks -> must fail!
    let bill_d = BillWithFallback {
        items_total: 100.0,
        discount: Some(10.0),
        tax: Some(5.0),
        total: 130.0, // Arbitrary invalid total
    };
    let res = bill_d.validate_integrity();
    assert!(res.is_err());
    let err = &res.unwrap_err()[0];
    assert_eq!(err.field_path, "total");
    assert!(err.error_message.contains("items_minus_discount"));
    assert!(err.error_message.contains("items_minus_tax"));
    assert!(err.error_message.contains("exact_items_sum"));
}

#[test]
fn test_birthdate_year_range_validation() {
    let valid = CustomerProfile {
        full_name: "Ada Lovelace".to_string(),
        birthdate: "1985-12-10".to_string(),
        account_tier: Some("VIP".to_string()),
    };
    assert!(valid.validate_integrity().is_ok());

    // Birthdate before 1920
    let too_old = CustomerProfile {
        full_name: "Historical Figure".to_string(),
        birthdate: "1850-05-01".to_string(),
        account_tier: None,
    };
    let errs = too_old.validate_integrity().unwrap_err();
    assert!(errs.iter().any(|e| e.field_path == "birthdate"));

    // Birthdate after 2026
    let future_born = CustomerProfile {
        full_name: "Future Child".to_string(),
        birthdate: "2040-01-01".to_string(),
        account_tier: None,
    };
    let errs_future = future_born.validate_integrity().unwrap_err();
    assert!(errs_future.iter().any(|e| e.field_path == "birthdate"));
}

#[test]
fn test_validation_outcome_modes_strict_soft_error() {
    // Case 1: Strict Result (passes all validation)
    let valid_profile = CustomerProfile {
        full_name: "Alan Turing".to_string(),
        birthdate: "1950-06-23".to_string(),
        account_tier: Some("Enterprise".to_string()),
    };
    let outcome_strict = ValidationOutcome::from_parsed(valid_profile.clone());
    assert!(outcome_strict.is_strict());
    assert!(!outcome_strict.is_soft());
    assert!(!outcome_strict.is_error());
    assert_eq!(outcome_strict.data(), Some(&valid_profile));
    assert!(outcome_strict.validation_errors().is_empty());

    // Case 2: Soft Result (parsed into data, but validation failed)
    let invalid_profile = CustomerProfile {
        full_name: "Time Traveler".to_string(),
        birthdate: "1800-01-01".to_string(), // Invalid year!
        account_tier: None,
    };
    let outcome_soft = ValidationOutcome::from_parsed(invalid_profile.clone());
    assert!(!outcome_soft.is_strict());
    assert!(outcome_soft.is_soft());
    assert!(!outcome_soft.is_error());
    assert_eq!(outcome_soft.data(), Some(&invalid_profile));
    assert_eq!(outcome_soft.validation_errors().len(), 1);
    assert_eq!(outcome_soft.validation_errors()[0].field_path, "birthdate");

    // Case 3: Error Result (unparseable / syntax error)
    let outcome_error: ValidationOutcome<CustomerProfile> = ValidationOutcome::Error {
        raw_output: "{ corrupt json ".to_string(),
        message: "EOF while parsing string".to_string(),
    };
    assert!(outcome_error.is_error());
    assert!(!outcome_error.is_strict());
    assert!(!outcome_error.is_soft());
    assert_eq!(outcome_error.data(), None);
}
