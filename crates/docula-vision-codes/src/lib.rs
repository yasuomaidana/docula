//! # docula-vision-codes
//!
//! Sovereign, pure-Rust optical code detection, 1D/2D barcode mapping, and
//! financial QR parsing for the `docula` document intelligence engine.
//!
//! Features:
//! - Multi-format barcode detection ([`OpticalCodeDetector`]).
//! - Mapped bounding box and finder pattern vertices ([`OpticalCodeResult`]).
//! - EPC SEPA, Swiss QR, and Mexican SAT CFDI financial parsers ([`FinancialQrPayload`]).

pub mod detector;
pub mod error;
pub mod parser;
pub mod types;

pub use detector::OpticalCodeDetector;
pub use error::OpticalCodeError;
pub use parser::parse_financial_payload;
pub use types::{
    FinancialQrPayload, MexicanSatCfdiPayload, OpticalCodeResult, OpticalCodeType, SepaEpcPayload,
    SwissQrPayload,
};

#[cfg(test)]
mod tests {
    use super::*;
    use docula_core::BoundingBox;
    use image::RgbaImage;

    #[test]
    fn test_detector_empty_image() {
        let detector = OpticalCodeDetector::new();
        let img = RgbaImage::new(100, 100);
        let results = detector.detect(&img).expect("detection should succeed");
        assert!(results.is_empty());
    }

    #[test]
    fn test_detector_zero_dimensions() {
        let detector = OpticalCodeDetector::new();
        let img = RgbaImage::new(0, 0);
        let err = detector.detect(&img).unwrap_err();
        assert!(matches!(
            err,
            OpticalCodeError::InvalidImageDimensions { .. }
        ));
    }

    #[test]
    fn test_types_and_reexports() {
        let result = OpticalCodeResult {
            code_type: OpticalCodeType::QrCode,
            raw_content: "HELLO".to_string(),
            bounding_box: BoundingBox::from_coords_clamped(0.1, 0.1, 0.2, 0.2),
            points: vec![],
            financial_payload: None,
        };
        assert_eq!(result.code_type, OpticalCodeType::QrCode);
        assert_eq!(result.raw_content, "HELLO");
    }
}
