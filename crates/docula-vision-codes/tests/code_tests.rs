use docula_vision_codes::{FinancialQrPayload, OpticalCodeDetector, OpticalCodeType};
use image::{Rgba, RgbaImage};
use rxing::{BarcodeFormat, MultiFormatWriter, Writer, common::BitMatrix};

fn bitmatrix_to_rgba(matrix: &BitMatrix) -> RgbaImage {
    let width = matrix.getWidth();
    let height = matrix.getHeight();
    let mut img = RgbaImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let pixel = if matrix.get(x, y) {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };
            img.put_pixel(x, y, pixel);
        }
    }
    img
}

#[test]
fn test_decode_sepa_epc_qr() {
    let epc_payload = "BCD\n001\n1\nSCT\nTESTBIC\nAcme Corp\nES1234567890123456789012\nEUR450.75\n\nRF18539007547034\nInvoice 1234\n";

    let writer = MultiFormatWriter;
    let bitmatrix = writer
        .encode(epc_payload, &BarcodeFormat::QR_CODE, 200, 200)
        .expect("QR encoding should succeed");

    let img = bitmatrix_to_rgba(&bitmatrix);
    let detector = OpticalCodeDetector::new();
    let results = detector.detect(&img).expect("detection should succeed");

    assert_eq!(results.len(), 1);
    let code = &results[0];
    assert_eq!(code.code_type, OpticalCodeType::QrCode);
    assert!(code.bounding_box.width() > 0.0);
    assert!(code.bounding_box.height() > 0.0);

    let financial = code
        .financial_payload
        .as_ref()
        .expect("should have financial payload");
    if let FinancialQrPayload::SepaEpc(sepa) = financial {
        assert_eq!(sepa.service_tag, "BCD");
        assert_eq!(sepa.beneficiary_name, "Acme Corp");
        assert_eq!(sepa.iban, "ES1234567890123456789012");
        assert_eq!(sepa.amount, Some(450.75));
        assert_eq!(sepa.currency, "EUR");
        assert_eq!(
            sepa.remittance_reference,
            Some("RF18539007547034".to_string())
        );
    } else {
        panic!("Expected SepaEpc payload variant");
    }
}

#[test]
fn test_decode_code128_barcode() {
    let text = "DOCULA99";
    let writer = MultiFormatWriter;
    let bitmatrix = writer
        .encode(text, &BarcodeFormat::CODE_128, 200, 80)
        .expect("Code 128 encoding should succeed");

    let img = bitmatrix_to_rgba(&bitmatrix);
    let detector = OpticalCodeDetector::new();
    let results = detector.detect(&img).expect("detection should succeed");

    assert!(!results.is_empty());
    let code = &results[0];
    assert_eq!(code.code_type, OpticalCodeType::Code128);
    assert_eq!(code.raw_content, text);
    assert!(code.bounding_box.width() > 0.0);
}

#[test]
fn test_empty_image_no_codes() {
    let img = RgbaImage::new(150, 150);
    let detector = OpticalCodeDetector::new();
    let results = detector.detect(&img).expect("empty detection ok");
    assert!(results.is_empty());
}
