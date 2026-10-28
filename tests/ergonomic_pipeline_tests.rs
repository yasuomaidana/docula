use docula::llm::MockLlmBackend;
use docula::schema::{DocuValidate, ValidationError};
use docula::{DoculaPipeline, DocumentSource};
use image::{ImageEncoder, Rgba, RgbaImage};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tempfile::NamedTempFile;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
struct SimpleReceipt {
    merchant: String,
    total: f64,
}

impl DocuValidate for SimpleReceipt {
    fn validate_integrity(&self) -> Result<(), Vec<ValidationError>> {
        if self.total < 0.0 {
            return Err(vec![ValidationError::new(
                "total",
                "Total cannot be negative",
            )]);
        }
        Ok(())
    }
}

fn create_test_image_bytes() -> Vec<u8> {
    let mut img = RgbaImage::new(100, 100);
    for pixel in img.pixels_mut() {
        *pixel = Rgba([240, 240, 240, 255]);
    }
    let mut bytes = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .expect("PNG encoding");
    bytes
}

#[tokio::test]
async fn test_ergonomic_process_infers_document_id_from_file_path() {
    // Use Case 2: Developer only has a file and a schema struct.
    let receipt = SimpleReceipt {
        merchant: "Acme Coffee".to_string(),
        total: 12.50,
    };
    let canned_json = serde_json::to_string(&receipt).unwrap();
    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let temp_file = NamedTempFile::with_prefix("receipt_coffee_101").unwrap();
    let path = temp_file.path().with_extension("png");
    std::fs::write(&path, create_test_image_bytes()).unwrap();

    let source = DocumentSource::from_image_path(&path);

    // Call ergonomic pipeline.process::<T>(&source) without needing to pass an artificial ID
    let result = pipeline
        .process::<SimpleReceipt>(&source)
        .await
        .expect("ergonomic extraction succeeds");

    assert_eq!(
        result.document_id,
        path.file_stem().unwrap().to_string_lossy(),
        "Inferred document_id must match the filename stem"
    );
    assert_eq!(result.data, Some(receipt));
    assert!(result.confidence >= 0.85);

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn test_ergonomic_process_infers_deterministic_id_from_bytes() {
    // Use Case 2: Developer has in-memory bytes and a schema struct.
    let receipt = SimpleReceipt {
        merchant: "Online Store".to_string(),
        total: 99.00,
    };
    let canned_json = serde_json::to_string(&receipt).unwrap();
    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let bytes = create_test_image_bytes();
    let source = DocumentSource::from_image_bytes(bytes, Some("png"));

    let result = pipeline
        .process::<SimpleReceipt>(&source)
        .await
        .expect("ergonomic extraction from bytes succeeds");

    assert!(
        result.document_id.starts_with("doc-"),
        "Inferred document_id for bytes must start with 'doc-'"
    );
    assert_eq!(result.document_id.len(), 20);
    assert_eq!(result.data, Some(receipt));
}

#[tokio::test]
async fn test_explicit_process_document_preserves_caller_id() {
    // Use Case 1: An external orchestrator / queue provides an explicit ID.
    let receipt = SimpleReceipt {
        merchant: "Warehouse Supplies".to_string(),
        total: 450.00,
    };
    let canned_json = serde_json::to_string(&receipt).unwrap();
    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let bytes = create_test_image_bytes();
    let source = DocumentSource::from_image_bytes(bytes, Some("png"));

    let explicit_id = "sqs-msg-order-987654";
    let result = pipeline
        .process_document::<SimpleReceipt>(explicit_id, &source)
        .await
        .expect("process_document succeeds with explicit ID");

    assert_eq!(
        result.document_id, explicit_id,
        "Explicit document_id must be strictly preserved"
    );
    assert_eq!(result.data, Some(receipt));
}
