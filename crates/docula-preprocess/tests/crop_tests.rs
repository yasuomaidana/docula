use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use docula_core::{BoundingBox, CropFormat, RegionCropper};
use docula_preprocess::DocumentVisualExtractor;
use pdf_oxide::api::Pdf;

fn generate_test_pdf_bytes(content: &str) -> Vec<u8> {
    let mut pdf = Pdf::from_html(content).expect("Failed to create test PDF from HTML");
    pdf.to_bytes().expect("Failed to convert PDF to bytes")
}

#[test]
fn test_capture_page_screenshot_png() {
    let pdf_bytes =
        generate_test_pdf_bytes("<h1>Screenshot Test</h1><p>Full page rendering in pure Rust</p>");
    let extractor = DocumentVisualExtractor::new(&pdf_bytes).with_dpi(150);

    let crop = extractor
        .capture_page(0, 150, CropFormat::Png)
        .expect("capture_page should succeed");

    assert_eq!(crop.page_index, 0);
    assert_eq!(crop.mime_type, "image/png");
    assert!(crop.width > 0);
    assert!(crop.height > 0);
    assert!(!crop.encoded_data.is_empty());

    let decoded_bytes = BASE64_STANDARD
        .decode(&crop.encoded_data)
        .expect("base64 payload must be valid");
    let decoded_img = image::load_from_memory(&decoded_bytes).expect("must decode to PNG image");
    assert_eq!(decoded_img.width(), crop.width);
    assert_eq!(decoded_img.height(), crop.height);
}

#[test]
fn test_crop_region_dimensions() {
    let pdf_bytes = generate_test_pdf_bytes(
        "<h1>Invoice Header</h1><p>Description: Widget A</p><p>Amount: $100.00</p>",
    );
    let extractor = DocumentVisualExtractor::new(&pdf_bytes).with_dpi(150);

    let bbox = BoundingBox::from_coords(0.1, 0.1, 0.5, 0.4).unwrap();
    let crop = extractor
        .crop_region(0, &bbox, 0.05, CropFormat::Png)
        .expect("crop_region should succeed");

    assert_eq!(crop.page_index, 0);
    assert!(crop.width > 0);
    assert!(crop.height > 0);
    assert_eq!(crop.mime_type, "image/png");

    let decoded_bytes = BASE64_STANDARD.decode(&crop.encoded_data).unwrap();
    let decoded_img = image::load_from_memory(&decoded_bytes).unwrap();
    assert_eq!(decoded_img.width(), crop.width);
    assert_eq!(decoded_img.height(), crop.height);
}

#[test]
fn test_crop_respects_censor_blackout() {
    let pdf_bytes =
        generate_test_pdf_bytes("<h1>Sensitive Bank Details</h1><p>IBAN: ES1234567890123456</p>");

    // Censor box in center
    let censor_box = BoundingBox::from_coords(0.1, 0.1, 0.4, 0.3).unwrap();
    let extractor = DocumentVisualExtractor::new(&pdf_bytes)
        .with_dpi(150)
        .with_censor_boxes(vec![censor_box]);

    // Crop the page region overlapping the censor box
    let crop_box = BoundingBox::from_coords(0.05, 0.05, 0.5, 0.5).unwrap();
    let crop = extractor
        .crop_region(0, &crop_box, 0.0, CropFormat::Png)
        .expect("crop should succeed");

    let decoded_bytes = BASE64_STANDARD.decode(&crop.encoded_data).unwrap();
    let decoded_img = image::load_from_memory(&decoded_bytes).unwrap().to_rgba8();

    // Verify that pixels within the censored portion of the crop are opaque black [0, 0, 0, 255]
    // The censor box (0.1..0.4, 0.1..0.3) relative to crop (0.05..0.5, 0.05..0.5):
    // Relative x = (0.2 - 0.05) / (0.5 - 0.05) = 0.15 / 0.45 = 1/3 across crop width
    let rel_x = ((0.2 - 0.05) / 0.45 * decoded_img.width() as f32) as u32;
    let rel_y = ((0.2 - 0.05) / 0.45 * decoded_img.height() as f32) as u32;

    let pixel = decoded_img.get_pixel(rel_x, rel_y);
    assert_eq!(
        pixel,
        &image::Rgba([0, 0, 0, 255]),
        "Censored region must be blacked out"
    );
}
