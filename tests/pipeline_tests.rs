use docula::llm::MockLlmBackend;
use docula::schema::{DocuValidate, ValidationError};
use docula::vision_codes::OpticalCodeType;
use docula::{DoculaPipeline, DocumentSource};
use image::{ImageEncoder, Rgba, RgbaImage};
use rxing::{BarcodeFormat, MultiFormatWriter, Writer};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
struct LineItem {
    description: String,
    amount: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
struct TestInvoice {
    invoice_number: String,
    items: Vec<LineItem>,
    total: f64,
}

impl DocuValidate for TestInvoice {
    fn validate_integrity(&self) -> Result<(), Vec<ValidationError>> {
        let sum: f64 = self.items.iter().map(|item| item.amount).sum();
        if (sum - self.total).abs() > 0.01 {
            return Err(vec![ValidationError::new(
                "total",
                format!("Sum of items ({sum}) does not match total ({})", self.total),
            )]);
        }
        Ok(())
    }
}

fn create_test_image_source() -> DocumentSource {
    let mut img = RgbaImage::new(200, 200);
    for pixel in img.pixels_mut() {
        *pixel = Rgba([255, 255, 255, 255]);
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

    DocumentSource::from_image_bytes(bytes, Some("png"))
}

#[tokio::test]
async fn test_pipeline_end_to_end_mock_success() {
    let invoice = TestInvoice {
        invoice_number: "INV-2026-001".to_string(),
        items: vec![
            LineItem {
                description: "Product A".to_string(),
                amount: 100.0,
            },
            LineItem {
                description: "Product B".to_string(),
                amount: 50.0,
            },
        ],
        total: 150.0,
    };
    let canned_json = serde_json::to_string(&invoice).unwrap();

    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let source = create_test_image_source();
    let result = pipeline
        .process_document::<TestInvoice>("doc-test-1", &source)
        .await
        .expect("pipeline execution must succeed");

    assert_eq!(result.document_id, "doc-test-1");
    assert_eq!(result.data, Some(invoice));
    assert!(result.confidence >= 0.85);
    assert!(!result.routed_to_triage);
    assert!(result.triage_reasons.is_empty());
    assert_eq!(result.repair_attempts, 1);
}

#[tokio::test]
async fn test_pipeline_self_repair_and_triage_on_failure() {
    // Arithmetic mismatch: items sum to 100.0, but total is 999.0
    let invalid_invoice = TestInvoice {
        invoice_number: "INV-FAIL".to_string(),
        items: vec![LineItem {
            description: "Widget".to_string(),
            amount: 100.0,
        }],
        total: 999.0,
    };
    let canned_json = serde_json::to_string(&invalid_invoice).unwrap();

    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let source = create_test_image_source();
    let result = pipeline
        .process_document::<TestInvoice>("doc-fail-1", &source)
        .await
        .expect("pipeline must succeed and route to triage");

    // Must have exhausted max repair attempts (3)
    assert_eq!(result.repair_attempts, 3);
    assert!(result.data.is_none());
    assert!(result.routed_to_triage);
    assert_eq!(pipeline.triage_queue().pending_count(), 1);
    assert!(!result.triage_reasons.is_empty());
    assert!(
        result
            .triage_reasons
            .iter()
            .any(|r| r.contains("Domain validation error")),
        "Expected domain validation reason, got: {:?}",
        result.triage_reasons
    );
}

#[tokio::test]
async fn test_pipeline_optical_code_enrichment() {
    let epc_payload = "BCD\n001\n1\nSCT\nTESTBIC\nAcme Corp\nES1234567890123456789012\nEUR450.75\n\nRF18539007547034\nInvoice 1234\n";
    let bitmatrix = MultiFormatWriter
        .encode(epc_payload, &BarcodeFormat::QR_CODE, 200, 200)
        .expect("QR encode");

    let width = bitmatrix.getWidth();
    let height = bitmatrix.getHeight();
    let mut img = RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let pixel = if bitmatrix.get(x, y) {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };
            img.put_pixel(x, y, pixel);
        }
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
    let source = DocumentSource::from_image_bytes(bytes, Some("png"));

    let empty_invoice = TestInvoice {
        invoice_number: "EPC-001".to_string(),
        items: vec![],
        total: 0.0,
    };
    let canned_json = serde_json::to_string(&empty_invoice).unwrap();

    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let result = pipeline
        .process_document::<TestInvoice>("doc-qr-1", &source)
        .await
        .expect("pipeline execution must succeed");

    assert_eq!(result.optical_codes.len(), 1);
    assert_eq!(result.optical_codes[0].code_type, OpticalCodeType::QrCode);
    assert!(result.optical_codes[0].financial_payload.is_some());
}

#[tokio::test]
async fn test_pipeline_rag_retrieval_enrichment_detected() {
    let invoice = TestInvoice {
        invoice_number: "INV-RAG-2026".to_string(),
        items: vec![LineItem {
            description: "Cloud Compute Dedicated Host".to_string(),
            amount: 750.0,
        }],
        total: 750.0,
    };
    let canned_json = serde_json::to_string(&invoice).unwrap();
    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));

    // Provide mock OCR engine with spatial spans so layout_map contains lines
    let spans = vec![docula::SpatialSpan::with_clamped_confidence(
        "INV-RAG-2026 Cloud Compute Dedicated Host Total 750.00",
        docula::BoundingBox::from_coords_clamped(0.1, 0.1, 0.9, 0.2),
        0,
        0.99,
    )];
    let mock_ocr = Arc::new(docula::ocr::MockOcrEngine::with_spans(spans));

    let config = docula::PipelineConfig {
        enable_rag: true,
        ..Default::default()
    };

    let pipeline = DoculaPipeline::builder()
        .with_config(config)
        .with_ocr_engine(mock_ocr)
        .with_llm_backend(mock_llm)
        .build();

    let source = create_test_image_source();
    let result = pipeline
        .process_document::<TestInvoice>("INV-RAG-2026", &source)
        .await
        .expect("pipeline execution with RAG enabled must succeed");

    // Detect and verify RAG grounding usage
    assert!(
        result.rag_applied,
        "RAG retrieval must be detected as applied"
    );
    assert!(
        !result.rag_retrieved_chunks.is_empty(),
        "Retrieved chunks must not be empty"
    );
    assert!(
        result.rag_retrieved_chunks[0]
            .text
            .contains("Cloud Compute Dedicated Host"),
        "Retrieved chunk must contain document terms"
    );
    assert_eq!(result.data, Some(invoice));
}
