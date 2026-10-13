use docula::{
    docula_model, DoculaPipeline, DocumentSource, ExtractionResult, ValidationMode,
};
use docula_llm::MockLlmBackend;
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder, Rgba, RgbaImage};
use std::sync::Arc;

docula_model! {
    pub struct CustomBillWithFallback {
        pub vendor: String,
        pub subtotal: f64,
        pub discount: Option<f64>,
        pub tax: Option<f64>,
        pub total: f64,
    }
    validate(self, v) {
        v.assert_non_empty("vendor", &self.vendor, "Vendor name");
        let disc = self.discount.unwrap_or(0.0);
        let tx = self.tax.unwrap_or(0.0);

        v.assert_fallback(
            "total",
            &[
                ("subtotal_minus_discount", (self.total - (self.subtotal - disc)).abs() <= 0.05),
                ("subtotal_minus_tax", (self.total - (self.subtotal - tx)).abs() <= 0.05),
                ("exact_subtotal", (self.total - self.subtotal).abs() <= 0.05),
            ],
            "Total must match subtotal - discount, subtotal - tax, or exact subtotal",
        );
    }
}

fn create_test_image_source() -> DocumentSource {
    let mut img = RgbaImage::new(100, 100);
    for pixel in img.pixels_mut() {
        *pixel = Rgba([255, 255, 255, 255]);
    }
    let mut bytes = Vec::new();
    let encoder = PngEncoder::new(&mut bytes);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ExtendedColorType::Rgba8,
        )
        .expect("PNG encoding");

    DocumentSource::from_image_bytes(bytes, Some("png"))
}

#[tokio::test]
async fn test_pipeline_strict_vs_soft_modes() {
    let source = create_test_image_source();

    // 1. Matches subtotal - tax (100 - 10 = 90), which is valid under fallback!
    let valid_json = serde_json::json!({
        "vendor": "Acme Global",
        "subtotal": 100.0,
        "discount": null,
        "tax": 10.0,
        "total": 90.0
    }).to_string();

    let mock_llm_valid = Arc::new(MockLlmBackend::with_canned_response(valid_json));
    let pipeline_strict = DoculaPipeline::builder()
        .with_llm_backend(mock_llm_valid)
        .build();

    let res: ExtractionResult<CustomBillWithFallback> = pipeline_strict
        .process_document("doc-strict-valid", &source)
        .await
        .expect("pipeline execution succeeds");

    // Strict pass!
    assert!(res.outcome.is_strict());
    assert!(res.data.is_some());
    assert_eq!(res.data.as_ref().unwrap().vendor, "Acme Global");
    assert!(!res.routed_to_triage);

    // 2. Corrupted total that fails fallback validation:
    let imbalanced_json = serde_json::json!({
        "vendor": "Acme Global",
        "subtotal": 100.0,
        "discount": null,
        "tax": 10.0,
        "total": 125.0
    }).to_string();

    // In Strict mode, imbalanced extraction yields None data and routes to triage:
    let mock_llm_imbalanced = Arc::new(MockLlmBackend::with_canned_response(imbalanced_json.clone()));
    let pipeline_strict_imbalanced = DoculaPipeline::builder()
        .with_llm_backend(mock_llm_imbalanced)
        .build();

    let res_strict_imbalanced: ExtractionResult<CustomBillWithFallback> = pipeline_strict_imbalanced
        .process_document("doc-strict-imbalanced", &source)
        .await
        .expect("pipeline execution succeeds");

    assert!(res_strict_imbalanced.outcome.is_soft());
    assert_eq!(res_strict_imbalanced.data, None); // Strict mode rejects data
    assert!(res_strict_imbalanced.routed_to_triage);
    assert_eq!(res_strict_imbalanced.outcome.validation_errors().len(), 1);

    // 3. In Soft mode, the user still receives the parsed struct with validation warnings!
    let mock_llm_soft = Arc::new(MockLlmBackend::with_canned_response(imbalanced_json));
    let config_soft = docula::PipelineConfig {
        validation_mode: ValidationMode::Soft,
        ..Default::default()
    };

    let pipeline_soft = DoculaPipeline::builder()
        .with_config(config_soft)
        .with_llm_backend(mock_llm_soft)
        .build();

    let res_soft: ExtractionResult<CustomBillWithFallback> = pipeline_soft
        .process_document("doc-soft-imbalanced", &source)
        .await
        .expect("pipeline execution succeeds");

    assert!(res_soft.outcome.is_soft());
    assert!(res_soft.data.is_some()); // Soft mode preserves parsed data!
    assert_eq!(res_soft.data.as_ref().unwrap().total, 125.0);
    assert_eq!(res_soft.outcome.validation_errors().len(), 1);
    assert_eq!(res_soft.outcome.validation_errors()[0].field_path, "total");
}

#[tokio::test]
async fn test_pipeline_syntax_error_outcome() {
    let source = create_test_image_source();

    let corrupt_json = "{ invalid json syntax !!".to_string();
    let mock_llm_corrupt = Arc::new(MockLlmBackend::with_raw_response(corrupt_json));

    let pipeline = DoculaPipeline::builder()
        .with_llm_backend(mock_llm_corrupt)
        .build();

    let res: ExtractionResult<CustomBillWithFallback> = pipeline
        .process_document("doc-error-syntax", &source)
        .await
        .expect("pipeline completes");

    assert!(res.outcome.is_error());
    assert_eq!(res.data, None);
    assert!(res.routed_to_triage);
}
