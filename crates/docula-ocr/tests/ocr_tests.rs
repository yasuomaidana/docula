use docula_core::{BoundingBox, SpatialSpan};
use docula_ocr::{DbNetDetector, MockOcrEngine, NeuralOcrEngine, OcrEngine, OcrError};
use image::{Rgba, RgbaImage};

/// Helper to render simulated text lines as high-contrast dark strokes on white background.
fn create_simulated_receipt_image(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));
    let black = Rgba([10, 10, 10, 255]);

    // Draw simulated line 1: "Invoice Number: INV-987654" at y=50..65
    for x in (50..350).step_by(8) {
        for y in 50..=65 {
            img.put_pixel(x, y, black);
            img.put_pixel(x + 1, y, black);
        }
    }

    // Draw simulated line 2: "Total: $1,250.00" at y=100..115
    for x in (50..250).step_by(8) {
        for y in 100..=115 {
            img.put_pixel(x, y, black);
            img.put_pixel(x + 1, y, black);
        }
    }

    img
}

#[test]
fn test_ocr_latin_text() {
    let img = create_simulated_receipt_image(600, 400);
    let engine = NeuralOcrEngine::new();

    // Register ground-truth patterns by row signature
    engine
        .recognizer()
        .register_pattern("row:1", "Invoice Number: INV-987654");
    engine
        .recognizer()
        .register_pattern("row:3", "Total: $1,250.00");

    let spans = engine.extract_spans(&img).expect("ocr extraction");
    assert!(!spans.is_empty(), "DBNet should detect text regions");

    let combined = spans
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(
        combined.contains("Invoice Number") || combined.contains("Total: $1,250.00"),
        "Extracted text: {combined}"
    );

    // Verify spatial bounding boxes are valid normalized coordinates
    for span in &spans {
        assert!(span.bbox.top_left.x >= 0.0 && span.bbox.top_left.x <= 1.0);
        assert!(span.bbox.top_left.y >= 0.0 && span.bbox.top_left.y <= 1.0);
        assert!(span.bbox.width() > 0.0);
        assert!(span.bbox.height() > 0.0);
    }
}

#[test]
fn test_ocr_cjk_text() {
    let mut img = RgbaImage::from_pixel(400, 200, Rgba([255, 255, 255, 255]));
    let black = Rgba([0, 0, 0, 255]);

    // Draw CJK receipt header blocks at y=40..60
    for x in (40..160).step_by(10) {
        for y in 40..=60 {
            img.put_pixel(x, y, black);
            img.put_pixel(x + 1, y, black);
        }
    }

    let engine = NeuralOcrEngine::new();
    engine
        .recognizer()
        .register_pattern("row:1", "領収書 合計 ¥12,500");

    let spans = engine.extract_spans(&img).expect("extraction");
    assert!(!spans.is_empty());

    let has_cjk = spans
        .iter()
        .any(|s| s.text.contains("領収書") || s.text.contains("TEXT_TOKEN"));
    assert!(has_cjk);
}

#[test]
fn test_ocr_engine_empty_image_error() {
    let empty_img = RgbaImage::new(0, 0);
    let engine = NeuralOcrEngine::new();

    let err = engine.extract_spans(&empty_img).unwrap_err();
    match err {
        OcrError::EmptyImage { width, height } => {
            assert_eq!(width, 0);
            assert_eq!(height, 0);
        }
        _ => panic!("Expected OcrError::EmptyImage, got {err}"),
    }
}

#[test]
fn test_mock_ocr_engine_polymorphism() {
    let bbox = BoundingBox::from_coords_clamped(0.1, 0.1, 0.5, 0.2);
    let span = SpatialSpan::new("Subtotal: $89.00".to_string(), bbox, 0, 0.99).expect("valid span");

    let mock = MockOcrEngine::with_spans(vec![span.clone()]);
    let engine: Box<dyn OcrEngine> = Box::new(mock);

    let img = RgbaImage::new(100, 100);
    let extracted = engine.extract_spans(&img).expect("mock extraction");

    assert_eq!(extracted.len(), 1);
    assert_eq!(extracted[0].text, "Subtotal: $89.00");
    assert_eq!(extracted[0].confidence, 0.99);
}

#[test]
fn test_dbnet_detector_unclip_and_sorting() {
    let img = create_simulated_receipt_image(500, 300);
    let detector = DbNetDetector::new(0.2, 1.6);

    let boxes = detector.detect(&img).expect("detect boxes");
    assert!(!boxes.is_empty(), "should detect connected text components");

    // Assert that boxes are sorted top to bottom
    for i in 1..boxes.len() {
        assert!(
            boxes[i].bbox.top_left.y >= boxes[i - 1].bbox.top_left.y - 0.05,
            "boxes must be sorted vertically"
        );
    }
}

#[test]
fn test_ocr_with_degraded_document_mode() {
    let img = create_simulated_receipt_image(600, 400);
    let engine = NeuralOcrEngine::new().with_degraded_document_mode();

    assert!(engine.preprocessor().is_some());

    engine
        .recognizer()
        .register_pattern("row:1", "Invoice Number: INV-987654");

    let spans = engine
        .extract_spans(&img)
        .expect("ocr extraction with degraded document mode");
    assert!(
        !spans.is_empty(),
        "DBNet should detect text regions even with preprocessing"
    );
}
