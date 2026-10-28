//! Contrast, unsharp mask, and Laplacian sharpening enhancements.

use crate::error::PreprocessError;
use crate::filter::ImageFilter;
use image::RgbaImage;
use oximedia_core::pixel_format::PixelFormat;
use oximedia_cv::enhance::sharpening::{UnsharpMaskParams, laplacian_sharpen, unsharp_mask};
use oxipix::mosaic::map_rgb_channels;

use super::denoise::{MorphologyDenoiseConfig, apply_morphology};
use super::denoise::{MorphologyOp, MorphologyShape};

/// Unsharp mask sharpening parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnsharpMaskConfig {
    pub radius: f64,
    pub amount: f64,
    pub threshold: f64,
}

impl Default for UnsharpMaskConfig {
    fn default() -> Self {
        Self {
            radius: 1.0,
            amount: 1.2,
            threshold: 2.0,
        }
    }
}

/// Preprocessing filter utilizing pure-Rust OxiMedia CV contrast, sharpening, and color compliance.
#[derive(Debug, Clone)]
pub struct OxiMediaFilter {
    pub contrast_factor: f32,
    pub enforce_color_compliance: bool,
    pub unsharp_mask: Option<UnsharpMaskConfig>,
    pub laplacian_strength: Option<f64>,
    pub morphological_denoise: Option<MorphologyDenoiseConfig>,
}

impl Default for OxiMediaFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl OxiMediaFilter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            contrast_factor: 1.2,
            enforce_color_compliance: true,
            unsharp_mask: None,
            laplacian_strength: None,
            morphological_denoise: None,
        }
    }

    #[must_use]
    pub fn with_contrast(mut self, factor: f32) -> Self {
        self.contrast_factor = factor.max(0.1);
        self
    }

    #[must_use]
    pub fn with_color_compliance(mut self, enforce: bool) -> Self {
        self.enforce_color_compliance = enforce;
        self
    }

    #[must_use]
    pub fn with_unsharp_mask(mut self, radius: f64, amount: f64, threshold: f64) -> Self {
        self.unsharp_mask = Some(UnsharpMaskConfig {
            radius: radius.max(0.1),
            amount: amount.max(0.0),
            threshold: threshold.max(0.0),
        });
        self
    }

    #[must_use]
    pub fn with_laplacian_sharpen(mut self, strength: f64) -> Self {
        self.laplacian_strength = Some(strength.max(0.0));
        self
    }

    #[must_use]
    pub fn with_morphological_denoise(
        mut self,
        op: MorphologyOp,
        shape: MorphologyShape,
        radius: usize,
    ) -> Self {
        self.morphological_denoise = Some(MorphologyDenoiseConfig {
            op,
            shape,
            radius: radius.max(1),
        });
        self
    }
}

impl ImageFilter for OxiMediaFilter {
    fn name(&self) -> &'static str {
        "OxiMediaCvFilter"
    }

    fn apply(&self, img: &mut RgbaImage) -> Result<(), PreprocessError> {
        let (width, height) = img.dimensions();
        if width == 0 || height == 0 {
            return Err(PreprocessError::InvalidDimensions {
                width,
                height,
                reason: "Dimensions must be non-zero for OxiMedia processing".to_string(),
            });
        }

        if self.enforce_color_compliance {
            let expected_depth = PixelFormat::Rgba32.bit_depth();
            if expected_depth != 8 {
                return Err(PreprocessError::FilterExecution {
                    filter: self.name(),
                    reason: format!("Unexpected pixel bit depth {expected_depth} for Rgba32"),
                });
            }
            let expected_len = (width as usize) * (height as usize) * 4;
            if img.as_raw().len() != expected_len {
                return Err(PreprocessError::FilterExecution {
                    filter: self.name(),
                    reason: format!(
                        "Pixel buffer size mismatch: expected {expected_len} bytes for Rgba32, got {}",
                        img.as_raw().len()
                    ),
                });
            }
        }

        if let Some(denoise_cfg) = self.morphological_denoise {
            apply_morphology(img, denoise_cfg)?;
        }

        if (self.contrast_factor - 1.0).abs() > 1e-4 {
            let mut total_lum = 0u64;
            let count = (width as u64) * (height as u64);
            for pixel in img.pixels() {
                let lum = (0.299 * pixel[0] as f32
                    + 0.587 * pixel[1] as f32
                    + 0.114 * pixel[2] as f32) as u64;
                total_lum += lum;
            }
            let mean_lum = (total_lum / count.max(1)) as f32;

            for pixel in img.pixels_mut() {
                for c in 0..3 {
                    let val = pixel[c] as f32;
                    let adjusted = mean_lum + (val - mean_lum) * self.contrast_factor;
                    pixel[c] = adjusted.clamp(0.0, 255.0) as u8;
                }
            }
        }

        if let Some(unsharp_cfg) = self.unsharp_mask {
            let params = UnsharpMaskParams {
                radius: unsharp_cfg.radius,
                amount: unsharp_cfg.amount,
                threshold: unsharp_cfg.threshold,
            };

            map_rgb_channels(img, |_c, chan, w, h| unsharp_mask(chan, w, h, &params));
        }

        if let Some(lap_strength) = self.laplacian_strength {
            map_rgb_channels(img, |_c, chan, w, h| {
                laplacian_sharpen(chan, w, h, lap_strength)
            });
        }

        Ok(())
    }
}
