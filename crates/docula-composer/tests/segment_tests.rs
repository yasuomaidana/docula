use docula_composer::{MultiDocumentSegmenter, SegmentationConfig};
use docula_core::DocumentSource;
use docula_preprocess::PageRasterBuffer;
use image::{ImageFormat, Rgba, RgbaImage};

/// Generates a test canvas on a dark gray surface (mimicking a table or desk)
/// with multiple bright rectangular document cards (mimicking receipts or invoices).
fn make_multi_document_scene(canvas_w: u32, canvas_h: u32) -> RgbaImage {
    // Dark background (desk/scanner background)
    let mut canvas = RgbaImage::from_pixel(canvas_w, canvas_h, Rgba([30, 30, 35, 255]));

    // Document 1: Top-Left receipt (e.g. at (20, 20), size 100x140)
    draw_document_card(&mut canvas, 20, 20, 100, 140, 240);

    // Document 2: Top-Right receipt (e.g. at (160, 25), size 90x130)
    draw_document_card(&mut canvas, 160, 25, 90, 130, 230);

    // Document 3: Bottom invoice (e.g. at (80, 180), size 120x90)
    draw_document_card(&mut canvas, 80, 180, 120, 90, 250);

    canvas
}

fn draw_document_card(canvas: &mut RgbaImage, x0: u32, y0: u32, w: u32, h: u32, base_luma: u8) {
    for y in y0..(y0 + h).min(canvas.height()) {
        for x in x0..(x0 + w).min(canvas.width()) {
            let p = canvas.get_pixel_mut(x, y);
            // Simulate document paper texture and printed text lines
            let is_text_line = (y % 14 < 3) && (x > x0 + 10 && x < x0 + w - 10);
            if is_text_line {
                p[0] = 40;
                p[1] = 40;
                p[2] = 40;
            } else {
                p[0] = base_luma;
                p[1] = base_luma;
                p[2] = base_luma;
            }
            p[3] = 255;
        }
    }
}

#[test]
fn test_segment_multi_documents_spatial_order_and_identity() {
    let canvas = make_multi_document_scene(300, 300);
    let buffer = PageRasterBuffer::new(0, canvas, 300).unwrap();

    let config = SegmentationConfig {
        min_area_ratio: 0.04,
        max_area_ratio: 0.80,
        target_dpi: 300,
        scan_optical_codes: false,
        preprocess_subdocs: false,
        luminance_threshold: 100,
        ..Default::default()
    };

    let segmenter = MultiDocumentSegmenter::new(config);
    let subdocs = segmenter
        .segment_from_buffer(&buffer)
        .expect("Multi-document segmentation must succeed");

    assert_eq!(
        subdocs.len(),
        3,
        "Should extract exactly 3 sub-documents from the scene"
    );

    // Verify spatial reading order: row 0 (top receipts) followed by row 1 (bottom receipt)
    for (expected_idx, subdoc) in subdocs.iter().enumerate() {
        assert_eq!(subdoc.identity.reading_order_index, expected_idx);
        assert!(
            subdoc
                .identity
                .instance_id
                .starts_with(&format!("subdoc-{expected_idx}-"))
        );
        assert_eq!(subdoc.identity.content_digest.len(), 64);
        assert!(subdoc.width > 50);
        assert!(subdoc.height > 50);
        assert!(subdoc.detection_confidence > 0.0);
    }

    // Verify DocumentSource conversion
    let src = subdocs[0]
        .to_source(ImageFormat::Png)
        .expect("to_source must succeed");
    assert!(src.is_image());
}

#[test]
fn test_segment_single_isolated_document() {
    let mut canvas = RgbaImage::from_pixel(200, 200, Rgba([20, 20, 20, 255]));
    draw_document_card(&mut canvas, 40, 40, 120, 120, 245);

    let segmenter = MultiDocumentSegmenter::new(SegmentationConfig {
        min_area_ratio: 0.05,
        luminance_threshold: 100,
        preprocess_subdocs: true,
        ..Default::default()
    });

    let subdocs = segmenter
        .segment_from_rgba(&canvas, 300)
        .expect("Single isolated document segmentation must succeed");

    assert_eq!(subdocs.len(), 1);
    let doc = &subdocs[0];
    assert_eq!(doc.identity.reading_order_index, 0);
    assert!(doc.width >= 100 && doc.width <= 130);
    assert!(doc.height >= 100 && doc.height <= 130);
}

#[test]
fn test_segment_fallback_to_canvas_when_no_subdocs_detected() {
    // Pure white canvas with no dark background
    let canvas = RgbaImage::from_pixel(150, 150, Rgba([255, 255, 255, 255]));

    let segmenter = MultiDocumentSegmenter::new(SegmentationConfig {
        min_area_ratio: 0.05,
        max_area_ratio: 0.90, // Canvas itself exceeds 0.90
        luminance_threshold: 128,
        ..Default::default()
    });

    let subdocs = segmenter
        .segment_from_rgba(&canvas, 300)
        .expect("Fallback segmentation must succeed");

    // Falls back gracefully to treating the full canvas as a single document instance
    assert_eq!(subdocs.len(), 1);
    assert_eq!(subdocs[0].width, 150);
    assert_eq!(subdocs[0].height, 150);
    assert_eq!(subdocs[0].identity.reading_order_index, 0);
}

#[test]
fn test_segment_from_document_source() {
    let canvas = make_multi_document_scene(300, 300);
    let mut png_bytes = Vec::new();
    canvas
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .unwrap();

    let source = DocumentSource::from_image_bytes(png_bytes, Some("png"));
    let segmenter = MultiDocumentSegmenter::new(SegmentationConfig {
        min_area_ratio: 0.04,
        luminance_threshold: 100,
        ..Default::default()
    });

    let subdocs = segmenter
        .segment_from_source(&source)
        .expect("Segment from DocumentSource must succeed");

    assert!(!subdocs.is_empty());
}
