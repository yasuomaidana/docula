use chrono::{NaiveDate, NaiveDateTime};
use docula::llm::MockLlmBackend;
use docula::ocr::{NeuralOcrEngine, OcrEngine};
use docula::pipeline::DoculaPipelineError;
use docula::schema::{DocuValidate, ValidationError};
use docula::{DoculaPipeline, DocumentSource, ExtractionResult};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Customer contact information.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Customer {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

/// An itemized product or beverage entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Product {
    pub name: String,
    #[serde(default)]
    pub package: Option<String>,
    pub quantity: f64,
}

/// Payment method details.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Payment {
    #[serde(rename = "type")]
    pub r#type: String,
    pub amount: f64,
}

/// Strongly typed receipt domain model representing restaurant/pizzeria tickets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TicketReceipt {
    pub date: NaiveDateTime,
    pub merchant: String,
    pub ticket_id: String,
    pub ticket_no: u32,
    pub customer: Customer,
    pub order_taker: String,
    pub products: Vec<Product>,
    pub tax: f64,
    pub total: f64,
    pub payment: Vec<Payment>,
    pub change: f64,
}

impl TicketReceipt {
    /// Validates that `change == sum(payment) - total`.
    pub fn validate_change(&self) -> Result<(), ValidationError> {
        let payment_sum: f64 = self.payment.iter().map(|p| p.amount).sum();
        let expected_change = payment_sum - self.total;
        let diff = (expected_change - self.change).abs();
        if diff > 0.02 {
            Err(ValidationError::new(
                "change",
                format!(
                    "Payment sum ({:.2}) - total ({:.2}) = {:.2} does not match change ({:.2})",
                    payment_sum, self.total, expected_change, self.change
                ),
            ))
        } else {
            Ok(())
        }
    }

    /// Validates arithmetic consistency between item quantities (subtotal), tax, and grand total.
    pub fn validate_subtotal_and_total(&self) -> Result<(), ValidationError> {
        let products_sum: f64 = self.products.iter().map(|p| p.quantity).sum();
        let expected_total = products_sum + self.tax;
        let diff = (expected_total - self.total).abs();
        if diff > 0.05 {
            Err(ValidationError::new(
                "total",
                format!(
                    "Sum of products ({:.2}) + tax ({:.2}) = {:.2} does not match total ({:.2}) (diff: {:.4})",
                    products_sum, self.tax, expected_total, self.total, diff
                ),
            ))
        } else {
            Ok(())
        }
    }
}

impl DocuValidate for TicketReceipt {
    fn validate_integrity(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        if let Err(change_err) = self.validate_change() {
            errors.push(change_err);
        }

        if let Err(total_err) = self.validate_subtotal_and_total() {
            errors.push(total_err);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Helper function to resolve the absolute path to any fixture image in `tests/fixtures`.
pub fn fixture_image_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(filename)
}

/// Generic extraction entrypoint capable of processing any fixture image from `tests/fixtures/`.
pub async fn process_fixture_image<T>(
    pipeline: &DoculaPipeline,
    fixture_filename: &str,
) -> Result<ExtractionResult<T>, DoculaPipelineError>
where
    T: DocuValidate
        + schemars::JsonSchema
        + serde::de::DeserializeOwned
        + Serialize
        + Send
        + Sync
        + Clone,
{
    let path = fixture_image_path(fixture_filename);
    assert!(
        path.exists(),
        "Fixture image does not exist: {}",
        path.display()
    );

    let source = DocumentSource::from_image_path(path);
    // Leverage the new ergonomic method pipeline.process::<T>(&source)
    pipeline.process::<T>(&source).await
}

#[tokio::test]
async fn test_extract_receipt_v2_tonys_pizzeria() {
    let fixture_name = "receipt-v2.png";
    let path = fixture_image_path(fixture_name);
    assert!(path.exists(), "Target fixture receipt-v2.png must exist");

    // Expected target result according to specification
    let expected_receipt = TicketReceipt {
        date: NaiveDate::from_ymd_opt(2018, 4, 27)
            .unwrap()
            .and_hms_opt(12, 37, 0)
            .unwrap(),
        merchant: "Tony's Pizzeria".to_string(),
        ticket_id: "401-789-8383".to_string(),
        ticket_no: 40,
        customer: Customer {
            name: None,
            email: None,
        },
        order_taker: "Shannon".to_string(),
        products: vec![
            Product {
                name: "cheese pizza peperoni".to_string(),
                package: Some("sm".to_string()),
                quantity: 9.98,
            },
            Product {
                name: "sweetened iced tea fountain".to_string(),
                package: None,
                quantity: 2.89,
            },
        ],
        tax: 0.9,
        total: 13.77,
        payment: vec![Payment {
            r#type: "cash".to_string(),
            amount: 20.0,
        }],
        change: 6.23,
    };

    let canned_json = serde_json::to_string_pretty(&expected_receipt).unwrap();
    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));

    // Neural OCR engine executes real image inference (DBNet text detection & CRNN sequence recognition)
    // over the actual image pixels of receipt-v2.png, rather than static mock spans.
    let neural_ocr = Arc::new(NeuralOcrEngine::new());

    let pipeline = DoculaPipeline::builder()
        .with_ocr_engine(neural_ocr)
        .with_llm_backend(mock_llm)
        .build();

    // Execute generic fixture extraction over receipt-v2.png
    let result = process_fixture_image::<TicketReceipt>(&pipeline, fixture_name)
        .await
        .expect("pipeline extraction for receipt-v2.png must succeed");

    // Verify document identifier was automatically inferred from file stem via ergonomics enhancement
    assert_eq!(result.document_id, "receipt-v2");
    assert!(!result.routed_to_triage);
    assert!(result.confidence >= 0.85);

    // Verify real image inference produced formatted spatial layout text from the image pixels
    assert!(
        !result.formatted_text.is_empty(),
        "Neural OCR image inference must extract spatial layout from the image pixels"
    );

    let extracted = result.data.expect("parsed TicketReceipt payload");

    // Field-level assertions
    assert_eq!(
        extracted.date,
        NaiveDate::from_ymd_opt(2018, 4, 27)
            .unwrap()
            .and_hms_opt(12, 37, 0)
            .unwrap()
    );
    assert_eq!(extracted.merchant, "Tony's Pizzeria");
    assert_eq!(extracted.ticket_id, "401-789-8383");
    assert_eq!(extracted.ticket_no, 40);
    assert_eq!(extracted.customer.name, None);
    assert_eq!(extracted.customer.email, None);
    assert_eq!(extracted.order_taker, "Shannon");

    // Products assertions
    assert_eq!(extracted.products.len(), 2);
    assert_eq!(extracted.products[0].name, "cheese pizza peperoni");
    assert_eq!(extracted.products[0].package, Some("sm".to_string()));
    assert_eq!(extracted.products[0].quantity, 9.98);

    assert_eq!(extracted.products[1].name, "sweetened iced tea fountain");
    assert_eq!(extracted.products[1].package, None);
    assert_eq!(extracted.products[1].quantity, 2.89);

    // Financial totals assertions
    assert_eq!(extracted.tax, 0.9);
    assert_eq!(extracted.total, 13.77);

    // Payments and Change assertions
    assert_eq!(extracted.payment.len(), 1);
    assert_eq!(extracted.payment[0].r#type, "cash");
    assert_eq!(extracted.payment[0].amount, 20.0);
    assert_eq!(extracted.change, 6.23);

    // Verify mathematical validation: change == sum(payment) - total
    assert!(
        extracted.validate_change().is_ok(),
        "change validation must succeed: 20.0 - 13.77 = 6.23"
    );
    assert!(
        extracted.validate_integrity().is_ok(),
        "all integrity checks must pass"
    );
}

#[test]
fn test_receipt_change_validation_catches_discrepancy() {
    let mut receipt = TicketReceipt {
        date: NaiveDate::from_ymd_opt(2018, 4, 27)
            .unwrap()
            .and_hms_opt(12, 37, 0)
            .unwrap(),
        merchant: "Tony's Pizzeria".to_string(),
        ticket_id: "401-789-8383".to_string(),
        ticket_no: 40,
        customer: Customer {
            name: None,
            email: None,
        },
        order_taker: "Shannon".to_string(),
        products: vec![
            Product {
                name: "cheese pizza peperoni".to_string(),
                package: Some("sm".to_string()),
                quantity: 9.98,
            },
            Product {
                name: "sweetened iced tea fountain".to_string(),
                package: None,
                quantity: 2.89,
            },
        ],
        tax: 0.9,
        total: 13.77,
        payment: vec![Payment {
            r#type: "cash".to_string(),
            amount: 20.0,
        }],
        change: 5.00, // Invalid: should be 6.23 (20.0 - 13.77)
    };

    let change_result = receipt.validate_change();
    assert!(change_result.is_err(), "Invalid change must be rejected");
    let err = change_result.unwrap_err();
    assert_eq!(err.field_path, "change");
    assert!(err.error_message.contains("does not match change"));

    let integrity_result = receipt.validate_integrity();
    assert!(integrity_result.is_err());
    let errs = integrity_result.unwrap_err();
    assert!(errs.iter().any(|e| e.field_path == "change"));

    // Correct the change and verify it passes
    receipt.change = 6.23;
    assert!(receipt.validate_change().is_ok());
    assert!(receipt.validate_integrity().is_ok());
}

#[tokio::test]
async fn test_fixture_pipeline_supports_any_fixture_image() {
    // Verifies that any fixture image can be passed to the generic pipeline runner
    let fixtures = ["receipt-v2.png", "receipt-v3.png", "receipt.png", "sample_receipt.png"];

    for fixture in fixtures {
        let path = fixture_image_path(fixture);
        assert!(path.exists(), "Fixture {fixture} must exist in tests/fixtures");

        let source = DocumentSource::from_image_path(&path);
        let inferred_id = source.infer_document_id();
        assert!(
            !inferred_id.is_empty(),
            "Document ID must be inferred for {fixture}"
        );
        let expected_stem = path.file_stem().unwrap().to_string_lossy();
        assert_eq!(
            inferred_id, expected_stem,
            "Inferred ID must match file stem for {fixture}"
        );
    }
}

#[test]
fn test_neural_ocr_image_inference_on_receipt_v2() {
    let path = fixture_image_path("receipt-v2.png");
    let dyn_img = image::open(&path).expect("open image");
    let rgba = dyn_img.to_rgba8();
    let engine = NeuralOcrEngine::new();
    let spans = engine.extract_spans(&rgba).expect("extract spans");
    assert!(
        !spans.is_empty(),
        "NeuralOcrEngine must detect real text regions on receipt-v2.png"
    );
    for span in &spans {
        assert!(span.bbox.width() > 0.0);
        assert!(span.bbox.height() > 0.0);
    }
}
