use docula_compliance::{ActionDirective, CensorPattern, ComplianceRedactor};
use docula_core::{BoundingBox, SpatialSpan};
use image::{Rgba, RgbaImage};

#[test]
fn test_blackout_boundary_pixels() {
    let mut image = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));

    // Blackout region: [0.2, 0.2] to [0.5, 0.5] -> pixels [20..50, 20..50]
    let bbox = BoundingBox::from_coords_clamped(0.2, 0.2, 0.5, 0.5);
    ComplianceRedactor::apply_blackout(&mut image, &bbox);

    // Pixels inside must be pure black
    for y in 20..50 {
        for x in 20..50 {
            assert_eq!(
                image.get_pixel(x, y),
                &Rgba([0, 0, 0, 255]),
                "pixel at ({x}, {y}) should be redacted black"
            );
        }
    }

    // Pixels outside must remain pure white
    assert_eq!(image.get_pixel(19, 20), &Rgba([255, 255, 255, 255]));
    assert_eq!(image.get_pixel(20, 19), &Rgba([255, 255, 255, 255]));
    assert_eq!(image.get_pixel(51, 50), &Rgba([255, 255, 255, 255]));
    assert_eq!(image.get_pixel(50, 51), &Rgba([255, 255, 255, 255]));
    assert_eq!(image.get_pixel(0, 0), &Rgba([255, 255, 255, 255]));
}

#[test]
fn test_purge_text_span_overlap() {
    let bbox_sensitive = BoundingBox::from_coords_clamped(0.1, 0.1, 0.4, 0.2);
    let bbox_safe = BoundingBox::from_coords_clamped(0.6, 0.6, 0.9, 0.7);
    let bbox_redact = BoundingBox::from_coords_clamped(0.05, 0.05, 0.45, 0.25);

    let span_sensitive =
        SpatialSpan::new("SSN: 123-45-6789".to_string(), bbox_sensitive, 0, 0.99).unwrap();
    let span_safe =
        SpatialSpan::new("Invoice Total: $450.00".to_string(), bbox_safe, 0, 0.98).unwrap();

    let mut spans = vec![span_sensitive, span_safe];

    ComplianceRedactor::purge_text_spans(&mut spans, &bbox_redact);

    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, "Invoice Total: $450.00");
}

#[test]
fn test_auto_censor_regex_iban() {
    let mut image = RgbaImage::from_pixel(200, 200, Rgba([255, 255, 255, 255]));
    let redactor = ComplianceRedactor::default();

    let bbox_iban = BoundingBox::from_coords_clamped(0.1, 0.1, 0.8, 0.2);
    let span_iban =
        SpatialSpan::new("GB82WEST12345698765432".to_string(), bbox_iban, 0, 0.95).unwrap();

    let bbox_safe = BoundingBox::from_coords_clamped(0.1, 0.5, 0.5, 0.6);
    let span_safe = SpatialSpan::new("Acme Corp".to_string(), bbox_safe, 0, 0.99).unwrap();

    let mut spans = vec![span_iban, span_safe];
    let redacted_count = redactor.auto_censor(&mut image, &mut spans);

    assert_eq!(redacted_count, 1);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, "Acme Corp");

    // Pixels inside IBAN area must be black
    assert_eq!(image.get_pixel(40, 30), &Rgba([0, 0, 0, 255]));
    // Pixels inside Acme Corp area must remain white
    assert_eq!(image.get_pixel(40, 110), &Rgba([255, 255, 255, 255]));
}

#[test]
fn test_auto_censor_regex_credit_card() {
    let mut image = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));
    let redactor = ComplianceRedactor::default();

    let bbox_cc = BoundingBox::from_coords_clamped(0.1, 0.1, 0.6, 0.3);
    let span_cc = SpatialSpan::new("4532 1122 3344 5566".to_string(), bbox_cc, 0, 0.95).unwrap();

    let mut spans = vec![span_cc];
    let redacted_count = redactor.auto_censor(&mut image, &mut spans);

    assert_eq!(redacted_count, 1);
    assert!(spans.is_empty());
    assert_eq!(image.get_pixel(30, 20), &Rgba([0, 0, 0, 255]));
}

#[test]
fn test_auto_censor_regex_ssn() {
    let mut image = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));
    let redactor = ComplianceRedactor::default();

    let bbox_ssn = BoundingBox::from_coords_clamped(0.2, 0.2, 0.6, 0.4);
    let span_ssn = SpatialSpan::new("987-65-4321".to_string(), bbox_ssn, 0, 0.99).unwrap();

    let mut spans = vec![span_ssn];
    let redacted_count = redactor.auto_censor(&mut image, &mut spans);

    assert_eq!(redacted_count, 1);
    assert!(spans.is_empty());
    assert_eq!(image.get_pixel(35, 30), &Rgba([0, 0, 0, 255]));
}

#[test]
fn test_action_directive_attributes() {
    use std::str::FromStr;

    assert_eq!(ActionDirective::Censor.as_str(), "censor");
    assert_eq!(ActionDirective::Ignore.as_str(), "ignore");
    assert_eq!(ActionDirective::NonStoring.as_str(), "non-storing");

    assert_eq!(
        ActionDirective::from_str("@censor").unwrap(),
        ActionDirective::Censor
    );
    assert_eq!(
        ActionDirective::from_str("ignore").unwrap(),
        ActionDirective::Ignore
    );
    assert_eq!(
        ActionDirective::from_str("@non-storing").unwrap(),
        ActionDirective::NonStoring
    );
    assert!(ActionDirective::from_str("unknown_dir").is_err());

    let custom_pattern =
        CensorPattern::new("custom_code", r"SECRET-\d{4}", ActionDirective::Censor).unwrap();
    assert!(custom_pattern.is_match("SECRET-1234"));
    assert!(!custom_pattern.is_match("PUBLIC-1234"));
}
