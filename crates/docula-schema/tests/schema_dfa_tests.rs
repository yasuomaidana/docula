use docula_schema::{
    DfaLogitsMasker, DocuValidate, FieldDirective, JsonSchema, SchemaDfa, ValidationError,
};
use serde::{Deserialize, Serialize};

#[derive(JsonSchema, Serialize, Deserialize, Debug, PartialEq)]
struct LineItem {
    description: String,
    quantity: u32,
    unit_price: f64,
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, PartialEq)]
struct Invoice {
    /// @censor
    tax_id: String,
    /// @crop
    signature: Option<String>,
    items: Vec<LineItem>,
    total: f64,
}

impl DocuValidate for Invoice {
    fn validate_integrity(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        if self.total < 0.0 {
            errors.push(ValidationError::new("total", "Total cannot be negative"));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[test]
fn test_dfa_nested_object_compilation() {
    let schema = schemars::schema_for!(Invoice);
    let dfa = SchemaDfa::compile(&schema).expect("compilation succeeds");

    assert_eq!(dfa.root_type(), "object");

    // Initial prefix permits opening brace '{' or leading whitespace
    assert!(dfa.can_transition("", "{"));
    assert!(dfa.can_transition("", "  {"));
    assert!(dfa.can_transition("", "\n{"));

    // Initial prefix disallows random letters or closing braces
    assert!(!dfa.can_transition("", "invoice"));
    assert!(!dfa.can_transition("", "}"));
    assert!(!dfa.can_transition("", "]"));
}

#[test]
fn test_dfa_invalid_transition() {
    let schema = schemars::schema_for!(Invoice);
    let dfa = SchemaDfa::compile(&schema).unwrap();

    let prefix = "{\n  \"total\": ";
    // Cannot transition with illegal JSON character outside quotes
    assert!(!dfa.can_transition(prefix, "}"));
    assert!(!dfa.can_transition(prefix, "$100"));

    let advance_result = dfa.advance(prefix, "$100");
    assert!(advance_result.is_err());
}

#[test]
fn test_masking_disallows_illegal_tokens() {
    let schema = schemars::schema_for!(Invoice);
    let dfa = SchemaDfa::compile(&schema).unwrap();
    let masker = DfaLogitsMasker::new(dfa);

    let vocab = vec![
        "{".to_string(),     // Valid (0)
        "}".to_string(),     // Invalid (1)
        "xyz".to_string(),   // Invalid (2)
        "123".to_string(),   // Invalid (3)
        "  {\n".to_string(), // Valid (4)
    ];

    let mut logits = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    masker.mask_logits("", &mut logits, &vocab);

    // Valid tokens retain scores
    assert_eq!(logits[0], 1.0);
    assert_eq!(logits[4], 5.0);

    // Invalid tokens are masked to -infinity
    assert_eq!(logits[1], f32::NEG_INFINITY);
    assert_eq!(logits[2], f32::NEG_INFINITY);
    assert_eq!(logits[3], f32::NEG_INFINITY);

    let valid_indices = masker.valid_token_indices("", &vocab);
    assert_eq!(valid_indices, vec![0, 4]);
}

#[test]
fn test_complete_token_sequence_yields_valid_json() {
    let schema = schemars::schema_for!(Invoice);
    let dfa = SchemaDfa::compile(&schema).unwrap();

    let tokens = vec![
        "{\n",
        "  \"tax_id\": \"US-9912\",\n",
        "  \"signature\": null,\n",
        "  \"items\": [],\n",
        "  \"total\": 0.0\n",
        "}",
    ];

    let mut prefix = String::new();
    for token in tokens {
        assert!(
            dfa.can_transition(&prefix, token),
            "Failed transition with token: {token:?} on prefix: {prefix:?}"
        );
        prefix.push_str(token);
    }

    assert!(dfa.is_accepted(&prefix));

    let parsed: Invoice = serde_json::from_str(&prefix).expect("valid JSON deserialization");
    assert_eq!(parsed.tax_id, "US-9912");
    assert_eq!(parsed.total, 0.0);
    assert!(parsed.items.is_empty());
}

#[test]
fn test_directive_registry_reflection() {
    let schema = schemars::schema_for!(Invoice);
    let dfa = SchemaDfa::compile(&schema).unwrap();

    let registry = dfa.directives();
    assert!(registry.has_directive("tax_id", &FieldDirective::Censor));
    assert!(registry.has_directive("signature", &FieldDirective::Crop));
    assert!(!registry.has_directive("total", &FieldDirective::Censor));
}

#[test]
fn test_docu_validate_trait() {
    let valid_inv = Invoice {
        tax_id: "US-1".to_string(),
        signature: None,
        items: vec![],
        total: 150.0,
    };
    assert!(valid_inv.validate_integrity().is_ok());

    let invalid_inv = Invoice {
        tax_id: "US-1".to_string(),
        signature: None,
        items: vec![],
        total: -50.0,
    };
    let errs = invalid_inv.validate_integrity().unwrap_err();
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].field_path, "total");
    assert!(errs[0].error_message.contains("negative"));
}
