use crate::error::OpticalCodeError;
use crate::parser::parse_financial_payload;
use crate::types::{OpticalCodeResult, OpticalCodeType};
use docula_core::{BoundingBox, Point};
use image::{DynamicImage, RgbaImage};
use rxing::{
    BarcodeFormat, BinaryBitmap, BufferedImageLuminanceSource, Exceptions, MultiFormatReader,
    ResultPoint,
    common::HybridBinarizer,
    multi::{GenericMultipleBarcodeReader, MultipleBarcodeReader},
};

/// Pure-Rust optical code detector scanning 1D barcodes and 2D matrix codes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpticalCodeDetector;

impl OpticalCodeDetector {
    /// Creates a new `OpticalCodeDetector`.
    #[inline]
    pub const fn new() -> Self {
        Self
    }

    /// Detects all 1D/2D optical codes in an RGBA pixel buffer.
    pub fn detect(&self, image: &RgbaImage) -> Result<Vec<OpticalCodeResult>, OpticalCodeError> {
        let dyn_img = DynamicImage::ImageRgba8(image.clone());
        self.detect_dynamic(&dyn_img)
    }

    /// Detects all 1D/2D optical codes in a [`DynamicImage`].
    pub fn detect_dynamic(
        &self,
        image: &DynamicImage,
    ) -> Result<Vec<OpticalCodeResult>, OpticalCodeError> {
        let (width, height) = (image.width(), image.height());
        if width == 0 || height == 0 {
            return Err(OpticalCodeError::InvalidImageDimensions { width, height });
        }

        let source = BufferedImageLuminanceSource::new(image.clone());
        let mut bitmap = BinaryBitmap::new(HybridBinarizer::new(source));
        let mut reader = GenericMultipleBarcodeReader::new(MultiFormatReader::default());

        let results = match reader.decode_multiple(&mut bitmap) {
            Ok(res) => res,
            Err(Exceptions::NotFoundException(_)) => return Ok(Vec::new()),
            Err(err) => {
                return Err(OpticalCodeError::DecodingFailure {
                    reason: err.to_string(),
                });
            }
        };

        let mut mapped = Vec::with_capacity(results.len());
        for res in results {
            let code_type = map_barcode_format(res.getBarcodeFormat());
            let raw_content = res.getText().to_string();
            let (points, bbox) = extract_points_and_bbox(res.getPoints(), width, height)?;
            let financial_payload = parse_financial_payload(&raw_content);

            mapped.push(OpticalCodeResult {
                code_type,
                raw_content,
                bounding_box: bbox,
                points,
                financial_payload,
            });
        }

        Ok(mapped)
    }
}

/// Maps an `rxing::BarcodeFormat` to domain [`OpticalCodeType`].
fn map_barcode_format(format: &BarcodeFormat) -> OpticalCodeType {
    match format {
        BarcodeFormat::QR_CODE => OpticalCodeType::QrCode,
        BarcodeFormat::DATA_MATRIX => OpticalCodeType::DataMatrix,
        BarcodeFormat::AZTEC => OpticalCodeType::Aztec,
        BarcodeFormat::PDF_417 => OpticalCodeType::Pdf417,
        BarcodeFormat::CODE_128 => OpticalCodeType::Code128,
        BarcodeFormat::CODE_39 => OpticalCodeType::Code39,
        BarcodeFormat::CODE_93 => OpticalCodeType::Code93,
        BarcodeFormat::EAN_13 => OpticalCodeType::Ean13,
        BarcodeFormat::EAN_8 => OpticalCodeType::Ean8,
        BarcodeFormat::UPC_A => OpticalCodeType::UpcA,
        BarcodeFormat::UPC_E => OpticalCodeType::UpcE,
        BarcodeFormat::CODABAR => OpticalCodeType::Codabar,
        BarcodeFormat::ITF => OpticalCodeType::Itf,
        other => OpticalCodeType::Other(format!("{other:?}")),
    }
}

/// Normalizes raw image coordinates into page-relative points and a [`BoundingBox`].
fn extract_points_and_bbox(
    raw_points: &[rxing::Point],
    width: u32,
    height: u32,
) -> Result<(Vec<Point>, BoundingBox), OpticalCodeError> {
    let w = width as f32;
    let h = height as f32;

    let mut points = Vec::with_capacity(raw_points.len());
    let mut min_x = 1.0f32;
    let mut min_y = 1.0f32;
    let mut max_x = 0.0f32;
    let mut max_y = 0.0f32;

    for pt in raw_points {
        let x = (pt.getX() / w).clamp(0.0, 1.0);
        let y = (pt.getY() / h).clamp(0.0, 1.0);
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);

        if let Ok(p) = Point::new(x, y) {
            points.push(p);
        }
    }

    if points.is_empty() {
        let bbox = BoundingBox::from_coords_clamped(0.0, 0.0, 1.0, 1.0);
        return Ok((points, bbox));
    }

    // Ensure non-zero width and height for line barcodes
    if (max_x - min_x).abs() < 1e-4 {
        max_x = (min_x + 0.02).min(1.0);
    }
    if (max_y - min_y).abs() < 1e-4 {
        max_y = (min_y + 0.02).min(1.0);
    }

    let bbox = BoundingBox::from_coords_clamped(min_x, min_y, max_x, max_y);
    Ok((points, bbox))
}
