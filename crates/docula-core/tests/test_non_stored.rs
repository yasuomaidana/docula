//! Integration tests for DOC-103: Zero-Storage RAII Smart Pointer `NonStored<T>`.
//!
//! Traceability: `docs/01_DEVELOPMENT_PLAN.md` (DOC-103) and
//! `docs/14_COMPLIANCE_AND_NON_STORING.md` §3 (`@non-storing`) and §5.1.

use docula_core::NonStored;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct PayrollStatement {
    employee_name: String,
    net_pay: f64,
    #[serde(default, skip_serializing_if = "NonStored::always_skip")]
    payroll_verification_token: NonStored<String>,
}

#[derive(Serialize)]
struct UnannotatedRecord {
    id: u32,
    token: NonStored<String>,
}

// AC1 / `test_non_stored_deref`
#[test]
fn test_non_stored_deref() {
    let token = NonStored::new("OTP-998877".to_owned());
    assert_eq!(token.len(), 10);
    assert!(token.starts_with("OTP-"));
    assert_eq!(&*token, "OTP-998877");
}

#[test]
fn test_non_stored_deref_mut() {
    let mut token = NonStored::new(String::from("abc"));
    token.push_str("def");
    assert_eq!(token.as_str(), "abcdef");
}

// AC3 / `test_non_stored_serde_skip`
#[test]
fn test_non_stored_serde_skip() {
    let statement = PayrollStatement {
        employee_name: "Ada Lovelace".to_owned(),
        net_pay: 4200.5,
        payroll_verification_token: NonStored::new("OTP-998877".to_owned()),
    };
    let json = serde_json::to_string(&statement).expect("serialize");
    assert!(
        !json.contains("payroll_verification_token"),
        "field must be absent: {json}"
    );
    assert!(
        !json.contains("OTP-998877"),
        "secret must never be emitted: {json}"
    );
    assert!(json.contains("Ada Lovelace"));
}

#[test]
fn test_non_stored_without_skip_attribute_serializes_null() {
    let record = UnannotatedRecord {
        id: 7,
        token: NonStored::new("secret".to_owned()),
    };
    let json = serde_json::to_string(&record).expect("serialize");
    assert_eq!(json, r#"{"id":7,"token":null}"#);
}

// AC4: deserialize from LLM JSON responses
#[test]
fn test_non_stored_deserializes_from_llm_json() {
    let llm_json = r#"{
        "employee_name": "Ada Lovelace",
        "net_pay": 4200.5,
        "payroll_verification_token": "OTP-998877"
    }"#;
    let statement: PayrollStatement = serde_json::from_str(llm_json).expect("deserialize");
    assert_eq!(statement.payroll_verification_token.as_str(), "OTP-998877");
}

#[test]
fn test_archived_json_round_trip_yields_empty_token() {
    let archived = r#"{"employee_name":"Ada","net_pay":1.0}"#;
    let statement: PayrollStatement = serde_json::from_str(archived).expect("deserialize");
    assert!(statement.payroll_verification_token.is_empty());
}

#[test]
fn test_non_stored_deserialize_type_mismatch_errors() {
    let result: Result<NonStored<u64>, _> = serde_json::from_str(r#""not-a-number""#);
    assert!(result.is_err());
}

/// Probe whose `zeroize` scrubs a fixed buffer and records the post-scrub state
/// into shared memory observable after the owner is dropped.
struct ScrubProbe {
    buffer: [u8; 16],
    observed: Rc<RefCell<Option<[u8; 16]>>>,
}

impl Zeroize for ScrubProbe {
    fn zeroize(&mut self) {
        self.buffer.zeroize();
        *self.observed.borrow_mut() = Some(self.buffer);
    }
}

// AC2 / `test_non_stored_zeroize_on_drop`
#[test]
fn test_non_stored_zeroize_on_drop() {
    let observed = Rc::new(RefCell::new(None));
    {
        let secret = NonStored::new(ScrubProbe {
            buffer: [0xAB; 16],
            observed: Rc::clone(&observed),
        });
        assert_eq!(secret.buffer, [0xAB; 16]);
        assert!(observed.borrow().is_none(), "must not scrub while alive");
    }
    assert_eq!(
        *observed.borrow(),
        Some([0u8; 16]),
        "buffer must be zeroed on drop"
    );
}

#[test]
fn test_non_stored_byte_buffer_zeroize_in_place() {
    let mut secret = NonStored::new(vec![1u8, 2, 3, 4]);
    secret.zeroize();
    assert!(secret.is_empty(), "Vec zeroize scrubs and truncates");
}

#[test]
fn test_non_stored_is_zeroize_on_drop() {
    fn assert_zod<T: ZeroizeOnDrop>() {}
    assert_zod::<NonStored<String>>();
    assert_zod::<NonStored<Vec<u8>>>();
}

#[test]
fn test_non_stored_debug_never_leaks() {
    let statement = PayrollStatement {
        employee_name: "Ada".to_owned(),
        net_pay: 1.0,
        payroll_verification_token: NonStored::new("OTP-998877".to_owned()),
    };
    let rendered = format!("{statement:?}");
    assert!(!rendered.contains("OTP-998877"));
    assert!(rendered.contains("<redacted>"));
}
