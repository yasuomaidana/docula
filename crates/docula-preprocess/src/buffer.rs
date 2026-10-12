use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use docula_core::{BoundingBox, CropError, CropFormat, ImageCrop};
use image::{DynamicImage, ImageEncoder, RgbaImage};

use crate::error::PreprocessError;

/// In-memory rasterized representation of a single document page.
///
/// Encapsulates an RGBA pixel buffer rendered at a specified DPI, providing
/// direct pixel access and sub-image ROI (region-of-interest) cropping.
#[derive(Clone, Debug, PartialEq)]
pub struct PageRasterBuffer {
    /// Source document page index (0-indexed).
    pub page_index: usize,
    /// In-memory RGBA pixel buffer.
    pub image: RgbaImage,
    /// Rasterization resolution in dots-per-inch.
    pub dpi: u32,
}

impl PageRasterBuffer {
    /// Creates a new `PageRasterBuffer`.
    pub fn new(page_index: usize, image: RgbaImage, dpi: u32) -> Result<Self, PreprocessError> {
        if dpi == 0 {
            return Err(PreprocessError::InvalidDpi { dpi });
        }
        Ok(Self {
            page_index,
            image,
            dpi,
        })
    }

    /// Returns the pixel width of the page buffer.
    #[inline]
    pub fn width(&self) -> u32 {
        self.image.width()
    }

    /// Returns the pixel height of the page buffer.
    #[inline]
    pub fn height(&self) -> u32 {
        self.image.height()
    }

    /// Returns the `(width, height)` pixel dimensions.
    #[inline]
    pub fn dimensions(&self) -> (u32, u32) {
        self.image.dimensions()
    }

    /// Returns a reference to the raw contiguous RGBA8 byte slice (`width * height * 4`).
    #[inline]
    pub fn as_raw(&self) -> &[u8] {
        self.image.as_raw()
    }

    /// Consumes the buffer and returns the underlying [`RgbaImage`].
    #[inline]
    pub fn into_image(self) -> RgbaImage {
        self.image
    }

    /// Crops a sub-image region corresponding to a normalized [`BoundingBox`].
    pub fn crop_region(&self, bbox: &BoundingBox) -> Result<RgbaImage, PreprocessError> {
        if self.width() == 0 || self.height() == 0 {
            return Err(CropError::ZeroDimensions {
                width: self.width(),
                height: self.height(),
            }
            .into());
        }

        let rect = bbox.to_pixel_rect(self.width(), self.height());
        let cropped_view =
            image::imageops::crop_imm(&self.image, rect.x, rect.y, rect.width, rect.height);
        Ok(cropped_view.to_image())
    }

    /// Crops a region and encodes it into an [`ImageCrop`] value object with Base64 payload.
    pub fn crop_to_image_crop(
        &self,
        bbox: &BoundingBox,
        format: CropFormat,
    ) -> Result<ImageCrop, PreprocessError> {
        let sub_img = self.crop_region(bbox)?;
        let width = sub_img.width();
        let height = sub_img.height();

        let encoded_bytes = encode_rgba_image(&sub_img, format)?;
        let encoded_data = BASE64_STANDARD.encode(&encoded_bytes);

        let crop = ImageCrop::new(
            self.page_index,
            *bbox,
            width,
            height,
            encoded_data,
            format.mime_type(),
        )?;

        Ok(crop)
    }
}

/// Helper function to encode an [`RgbaImage`] into byte representation for the target format.
fn encode_rgba_image(img: &RgbaImage, format: CropFormat) -> Result<Vec<u8>, PreprocessError> {
    let mut bytes = Vec::new();
    match format {
        CropFormat::RawRgba => {
            bytes.extend_from_slice(img.as_raw());
        }
        CropFormat::Png => {
            let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
            encoder
                .write_image(
                    img.as_raw(),
                    img.width(),
                    img.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(|e| PreprocessError::ImageEncodingError {
                    reason: e.to_string(),
                })?;
        }
        CropFormat::Jpeg { quality } => {
            let rgb = DynamicImage::ImageRgba8(img.clone()).to_rgb8();
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality);
            encoder
                .encode(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    image::ExtendedColorType::Rgb8,
                )
                .map_err(|e| PreprocessError::ImageEncodingError {
                    reason: e.to_string(),
                })?;
        }
        CropFormat::WebP => {
            return Err(PreprocessError::ImageEncodingError {
                reason: "WebP encoding format is not currently supported".to_string(),
            });
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_creation_and_properties() {
        let img = RgbaImage::new(100, 200);
        let buffer = PageRasterBuffer::new(0, img, 150).expect("valid buffer");
        assert_eq!(buffer.width(), 100);
        assert_eq!(buffer.height(), 200);
        assert_eq!(buffer.dimensions(), (100, 200));
        assert_eq!(buffer.as_raw().len(), 100 * 200 * 4);
        assert_eq!(buffer.dpi, 150);

        let consumed = buffer.into_image();
        assert_eq!(consumed.dimensions(), (100, 200));
    }

    #[test]
    fn test_buffer_invalid_dpi() {
        let img = RgbaImage::new(10, 10);
        let err = PageRasterBuffer::new(0, img, 0).unwrap_err();
        assert!(matches!(err, PreprocessError::InvalidDpi { dpi: 0 }));
    }

    #[test]
    fn test_crop_region_zero_dimensions() {
        let img = RgbaImage::new(0, 0);
        let buffer = PageRasterBuffer::new(0, img, 150).expect("valid buffer");
        let bbox = BoundingBox::from_coords(0.1, 0.1, 0.5, 0.5).unwrap();
        let err = buffer.crop_region(&bbox).unwrap_err();
        assert!(matches!(
            err,
            PreprocessError::Crop(CropError::ZeroDimensions { .. })
        ));
    }

    #[test]
    fn test_encode_rgba_image_unsupported_webp() {
        let img = RgbaImage::new(10, 10);
        let err = encode_rgba_image(&img, CropFormat::WebP).unwrap_err();
        assert!(matches!(err, PreprocessError::ImageEncodingError { .. }));
    }
}
