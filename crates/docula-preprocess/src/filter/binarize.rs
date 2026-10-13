use super::ImageFilter;
use crate::error::PreprocessError;
use image::{GrayImage, Luma, Rgba, RgbaImage};
use imageproc::contrast::{ThresholdType, adaptive_threshold, otsu_level, threshold};

/// Binarization algorithm selection for [`AdaptiveBinarizeFilter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinarizeMethod {
    /// Local adaptive mean thresholding with configurable sliding window radius.
    Adaptive { block_radius: u32 },
    /// Global Otsu clustering threshold.
    Otsu,
}

/// Adaptive binarization filter for contrast normalization and shadow removal.
///
/// Converts a document image into high-contrast black and white. Ideal for removing
/// lighting gradients and uneven scanner illumination before OCR line detection.
#[derive(Debug, Clone, Copy)]
pub struct AdaptiveBinarizeFilter {
    method: BinarizeMethod,
}

impl Default for AdaptiveBinarizeFilter {
    fn default() -> Self {
        Self::adaptive(12)
    }
}

impl AdaptiveBinarizeFilter {
    /// Creates an adaptive threshold filter with the specified sliding window block radius.
    #[must_use]
    pub fn new(block_radius: u32) -> Self {
        Self::adaptive(block_radius)
    }

    /// Creates an adaptive threshold filter with the specified block radius.
    #[must_use]
    pub fn adaptive(block_radius: u32) -> Self {
        Self {
            method: BinarizeMethod::Adaptive { block_radius },
        }
    }

    /// Creates a global Otsu threshold filter.
    #[must_use]
    pub fn otsu() -> Self {
        Self {
            method: BinarizeMethod::Otsu,
        }
    }

    /// Returns the currently configured method.
    #[must_use]
    pub fn method(&self) -> BinarizeMethod {
        self.method
    }
}

impl ImageFilter for AdaptiveBinarizeFilter {
    fn name(&self) -> &'static str {
        "AdaptiveBinarizeFilter"
    }

    fn apply(&self, img: &mut RgbaImage) -> Result<(), PreprocessError> {
        let (width, height) = img.dimensions();
        if width == 0 || height == 0 {
            return Err(PreprocessError::InvalidDimensions {
                width,
                height,
                reason: "cannot binarize empty image".to_string(),
            });
        }

        // Convert RGBA to 8bpp grayscale
        let mut gray = GrayImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let pixel = img.get_pixel(x, y);
                let luma = (u32::from(pixel[0]) * 299
                    + u32::from(pixel[1]) * 587
                    + u32::from(pixel[2]) * 114)
                    / 1000;
                gray.put_pixel(x, y, Luma([luma as u8]));
            }
        }

        let binary_gray = match self.method {
            BinarizeMethod::Adaptive { block_radius } => {
                if block_radius == 0 {
                    return Err(PreprocessError::FilterExecution {
                        filter: self.name(),
                        reason: "block_radius must be positive".to_string(),
                    });
                }
                adaptive_threshold(&gray, block_radius)
            }
            BinarizeMethod::Otsu => {
                let level = otsu_level(&gray);
                threshold(&gray, level, ThresholdType::Binary)
            }
        };

        // Write binary result back into RGBA image (white = 255, black = 0)
        for y in 0..height {
            for x in 0..width {
                let val = binary_gray.get_pixel(x, y)[0];
                img.put_pixel(x, y, Rgba([val, val, val, 255]));
            }
        }

        Ok(())
    }
}
