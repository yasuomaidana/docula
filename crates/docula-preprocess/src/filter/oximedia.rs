//! OxiMedia pure-Rust computer vision integration for docula preprocessing.
//!
//! Wraps the KitaSan `oximedia`, `oximedia-core`, and `oximedia-cv` framework
//! to provide sovereign, patent-free image processing, filtering, and computer vision
//! transformations without any OpenCV/FFmpeg dynamic C/C++ dependencies.

use crate::error::PreprocessError;
use crate::filter::ImageFilter;
use image::{Rgba, RgbaImage};
use oximedia_core::pixel_format::PixelFormat;
use oximedia_cv::enhance::sharpening::{UnsharpMaskParams, laplacian_sharpen, unsharp_mask};
use oximedia_cv::image::filter::{ImageFilter as OxiCvFilter, MedianFilter};
use oximedia_cv::morphology::{StructuringElement, StructuringElementShape, closing, opening};

/// Unsharp mask sharpening parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnsharpMaskConfig {
    /// Gaussian blur radius (sigma in pixels, > 0).
    pub radius: f64,
    /// Sharpening strength multiplier (1.0 = standard).
    pub amount: f64,
    /// Minimum luminance difference threshold before sharpening is applied.
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

/// Structuring element shape for mathematical morphology operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MorphologyShape {
    /// Rectangular kernel.
    #[default]
    Rectangle,
    /// Cross-shaped kernel (preserves diagonals).
    Cross,
    /// Elliptical / circular kernel.
    Ellipse,
}

impl From<MorphologyShape> for StructuringElementShape {
    fn from(shape: MorphologyShape) -> Self {
        match shape {
            MorphologyShape::Rectangle => Self::Rectangle,
            MorphologyShape::Cross => Self::Cross,
            MorphologyShape::Ellipse => Self::Ellipse,
        }
    }
}

/// Morphological denoising operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MorphologyOp {
    /// Opening: removes small bright spots / fills stroke gaps.
    Opening,
    /// Closing: removes small dark specks (pepper noise).
    Closing,
    /// Full restoration: impulsive noise filtering followed by stroke healing.
    #[default]
    FullRestoration,
}

/// Configuration for morphological noise reduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MorphologyDenoiseConfig {
    /// Target morphological operation.
    pub op: MorphologyOp,
    /// Kernel shape.
    pub shape: MorphologyShape,
    /// Radius of the structuring element (kernel dimension = `2 * radius + 1`).
    pub radius: usize,
}

impl Default for MorphologyDenoiseConfig {
    fn default() -> Self {
        Self {
            op: MorphologyOp::FullRestoration,
            shape: MorphologyShape::Rectangle,
            radius: 1,
        }
    }
}

/// Preprocessing filter utilizing the pure-Rust OxiMedia CV algorithms.
///
/// Applies sovereign image enhancements: contrast stretching, luminance normalization,
/// unsharp mask edge restoration, and morphological denoising powered by KitaSan `oximedia`.
#[derive(Debug, Clone)]
pub struct OxiMediaFilter {
    /// Target contrast threshold multiplier (default: 1.2).
    pub contrast_factor: f32,
    /// Whether to enforce strict RGB color profile compliance via OxiMedia Core.
    pub enforce_color_compliance: bool,
    /// Optional unsharp mask sharpening configuration.
    pub unsharp_mask: Option<UnsharpMaskConfig>,
    /// Optional Laplacian edge enhancement strength.
    pub laplacian_strength: Option<f64>,
    /// Optional morphological noise reduction configuration.
    pub morphological_denoise: Option<MorphologyDenoiseConfig>,
}

impl Default for OxiMediaFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl OxiMediaFilter {
    /// Creates a new `OxiMediaFilter` with default settings.
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

    /// Sets the contrast adjustment factor.
    #[must_use]
    pub fn with_contrast(mut self, factor: f32) -> Self {
        self.contrast_factor = factor.max(0.1);
        self
    }

    /// Enables or disables strict color compliance enforcement via `oximedia-core`.
    #[must_use]
    pub fn with_color_compliance(mut self, enforce: bool) -> Self {
        self.enforce_color_compliance = enforce;
        self
    }

    /// Configures unsharp mask sharpening for deblurring and text edge restoration.
    #[must_use]
    pub fn with_unsharp_mask(mut self, radius: f64, amount: f64, threshold: f64) -> Self {
        self.unsharp_mask = Some(UnsharpMaskConfig {
            radius: radius.max(0.1),
            amount: amount.max(0.0),
            threshold: threshold.max(0.0),
        });
        self
    }

    /// Configures Laplacian edge enhancement.
    #[must_use]
    pub fn with_laplacian_sharpen(mut self, strength: f64) -> Self {
        self.laplacian_strength = Some(strength.max(0.0));
        self
    }

    /// Configures morphological noise reduction and stroke repair.
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

        // 1. Enforce color compliance and pixel layout via OxiMedia Core
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

        let w = width as usize;
        let h = height as usize;

        // 2. Morphological noise reduction pass (if configured)
        if let Some(denoise_cfg) = self.morphological_denoise {
            apply_morphology(img, denoise_cfg)?;
        }

        // 3. Contrast expansion centered around mean luminance
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

        // 4. Unsharp mask sharpening via oximedia-cv enhance
        if let Some(unsharp_cfg) = self.unsharp_mask {
            let params = UnsharpMaskParams {
                radius: unsharp_cfg.radius,
                amount: unsharp_cfg.amount,
                threshold: unsharp_cfg.threshold,
            };

            for c in 0..3 {
                let mut channel_buf = Vec::with_capacity(w * h);
                for y in 0..height {
                    for x in 0..width {
                        channel_buf.push(img.get_pixel(x, y)[c]);
                    }
                }
                let sharpened = unsharp_mask(&channel_buf, w, h, &params);
                for y in 0..height {
                    for x in 0..width {
                        let idx = (y as usize) * w + (x as usize);
                        img.get_pixel_mut(x, y)[c] = sharpened[idx];
                    }
                }
            }
        }

        // 5. Laplacian sharpening via oximedia-cv enhance (if configured)
        if let Some(lap_strength) = self.laplacian_strength {
            for c in 0..3 {
                let mut channel_buf = Vec::with_capacity(w * h);
                for y in 0..height {
                    for x in 0..width {
                        channel_buf.push(img.get_pixel(x, y)[c]);
                    }
                }
                let sharpened = laplacian_sharpen(&channel_buf, w, h, lap_strength);
                for y in 0..height {
                    for x in 0..width {
                        let idx = (y as usize) * w + (x as usize);
                        img.get_pixel_mut(x, y)[c] = sharpened[idx];
                    }
                }
            }
        }

        Ok(())
    }
}

/// Denoise mode selection for [`OxiMediaDenoiseFilter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenoiseMode {
    /// Removes isolated dark specks on bright backgrounds (morphological closing).
    PepperRemoval { radius: usize },
    /// Removes isolated bright holes inside dark strokes (morphological opening).
    SaltRemoval { radius: usize },
    /// Removes both salt and pepper noise in degraded document scans.
    SaltAndPepper { radius: usize },
    /// Comprehensive restoration: impulsive noise removal followed by stroke healing.
    FullRestoration { radius: usize },
}

/// Dedicated document noise reduction and restoration filter utilizing OxiMedia CV morphology.
///
/// Designed specifically for degraded documents with severe noise, scanner artifacts,
/// salt-and-pepper distortion, and broken stroke lines.
#[derive(Debug, Clone)]
pub struct OxiMediaDenoiseFilter {
    /// Denoising mode.
    pub mode: DenoiseMode,
    /// Structuring element kernel shape.
    pub shape: MorphologyShape,
}

impl Default for OxiMediaDenoiseFilter {
    fn default() -> Self {
        Self::salt_and_pepper(1)
    }
}

impl OxiMediaDenoiseFilter {
    /// Creates a filter with explicit denoise mode and rectangular structuring element.
    #[must_use]
    pub fn new(mode: DenoiseMode) -> Self {
        Self {
            mode,
            shape: MorphologyShape::Rectangle,
        }
    }

    /// Convenience constructor for salt-and-pepper noise removal.
    #[must_use]
    pub fn salt_and_pepper(radius: usize) -> Self {
        Self::new(DenoiseMode::SaltAndPepper {
            radius: radius.max(1),
        })
    }

    /// Convenience constructor for scanner pepper speck removal.
    #[must_use]
    pub fn pepper_removal(radius: usize) -> Self {
        Self::new(DenoiseMode::PepperRemoval {
            radius: radius.max(1),
        })
    }

    /// Convenience constructor for stroke hole filling (salt removal).
    #[must_use]
    pub fn salt_removal(radius: usize) -> Self {
        Self::new(DenoiseMode::SaltRemoval {
            radius: radius.max(1),
        })
    }

    /// Convenience constructor for full document noise restoration.
    #[must_use]
    pub fn full_restoration(radius: usize) -> Self {
        Self::new(DenoiseMode::FullRestoration {
            radius: radius.max(1),
        })
    }

    /// Sets the kernel structuring element shape.
    #[must_use]
    pub fn with_shape(mut self, shape: MorphologyShape) -> Self {
        self.shape = shape;
        self
    }
}

impl ImageFilter for OxiMediaDenoiseFilter {
    fn name(&self) -> &'static str {
        "OxiMediaDenoiseFilter"
    }

    fn apply(&self, img: &mut RgbaImage) -> Result<(), PreprocessError> {
        let (width, height) = img.dimensions();
        if width == 0 || height == 0 {
            return Err(PreprocessError::InvalidDimensions {
                width,
                height,
                reason: "Dimensions must be non-zero for OxiMedia denoising".to_string(),
            });
        }

        let (op, radius) = match self.mode {
            DenoiseMode::PepperRemoval { radius } => (MorphologyOp::Closing, radius),
            DenoiseMode::SaltRemoval { radius } => (MorphologyOp::Opening, radius),
            DenoiseMode::SaltAndPepper { radius } | DenoiseMode::FullRestoration { radius } => {
                (MorphologyOp::FullRestoration, radius)
            }
        };

        let cfg = MorphologyDenoiseConfig {
            op,
            shape: self.shape,
            radius,
        };

        apply_morphology(img, cfg)
    }
}

/// Applies mathematical morphology and median filtering to an RGBA image buffer.
fn apply_morphology(
    img: &mut RgbaImage,
    cfg: MorphologyDenoiseConfig,
) -> Result<(), PreprocessError> {
    let (width, height) = img.dimensions();
    let w = width as usize;
    let h = height as usize;

    // For FullRestoration / SaltAndPepper: first apply OxiMedia MedianFilter to eliminate
    // impulsive isolated single-pixel salt and pepper noise without eroding thin strokes
    if cfg.op == MorphologyOp::FullRestoration {
        apply_median_3x3(img)?;
    }

    let k_size = 2 * cfg.radius + 1;
    let se_shape: StructuringElementShape = cfg.shape.into();
    let se = StructuringElement::from_shape(se_shape, k_size, k_size);

    for c in 0..3 {
        let mut float_buf = Vec::with_capacity(w * h);
        for y in 0..height {
            for x in 0..width {
                float_buf.push(f64::from(img.get_pixel(x, y)[c]));
            }
        }

        let processed = match cfg.op {
            MorphologyOp::Opening => opening(&float_buf, w, h, &se),
            MorphologyOp::Closing => closing(&float_buf, w, h, &se),
            MorphologyOp::FullRestoration => {
                // After median filtering eliminates impulsive noise,
                // opening heals text strokes and connects fine details
                opening(&float_buf, w, h, &se)
            }
        };

        for y in 0..height {
            for x in 0..width {
                let idx = (y as usize) * w + (x as usize);
                img.get_pixel_mut(x, y)[c] = processed[idx].round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    Ok(())
}

/// Applies a 3x3 median filter across RGB channels using `oximedia-cv` to strip isolated impulsive noise.
fn apply_median_3x3(img: &mut RgbaImage) -> Result<(), PreprocessError> {
    let (width, height) = img.dimensions();
    if width < 3 || height < 3 {
        return Ok(());
    }

    let filter = MedianFilter::new(3);

    for c in 0..3 {
        let mut channel_buf = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            for x in 0..width {
                channel_buf.push(img.get_pixel(x, y)[c]);
            }
        }

        let filtered = filter.apply(&channel_buf, width, height).map_err(|e| {
            PreprocessError::FilterExecution {
                filter: "OxiMediaDenoiseFilter",
                reason: e.to_string(),
            }
        })?;

        for y in 0..height {
            for x in 0..width {
                let idx = (y as usize) * (width as usize) + (x as usize);
                img.get_pixel_mut(x, y)[c] = filtered[idx];
            }
        }
    }

    Ok(())
}

/// Pure-Rust image stitcher utilizing OxiMedia computer vision primitives.
pub struct OxiMediaStitcher;

impl OxiMediaStitcher {
    /// Merges two vertically overlapping document image sections into a unified canvas.
    ///
    /// # Errors
    /// Returns [`PreprocessError`] if buffers are empty or dimensions mismatch.
    pub fn stitch_vertical(
        top: &RgbaImage,
        bottom: &RgbaImage,
        overlap_height: u32,
    ) -> Result<RgbaImage, PreprocessError> {
        let (w_top, h_top) = top.dimensions();
        let (w_bottom, h_bottom) = bottom.dimensions();

        if w_top == 0 || h_top == 0 || w_bottom == 0 || h_bottom == 0 {
            return Err(PreprocessError::InvalidDimensions {
                width: w_top,
                height: h_top,
                reason: "Image dimensions must be non-zero for vertical stitching".to_string(),
            });
        }

        let unified_width = w_top.max(w_bottom);
        let actual_overlap = overlap_height.min(h_top).min(h_bottom);
        let unified_height = h_top + h_bottom - actual_overlap;

        let mut canvas =
            RgbaImage::from_pixel(unified_width, unified_height, Rgba([255, 255, 255, 255]));

        // Copy top image
        for y in 0..h_top {
            for x in 0..w_top {
                canvas.put_pixel(x, y, *top.get_pixel(x, y));
            }
        }

        // Blend overlapping seam and append bottom image
        let seam_start = h_top - actual_overlap;
        for y in 0..h_bottom {
            let canvas_y = seam_start + y;
            for x in 0..w_bottom {
                let bottom_pixel = bottom.get_pixel(x, y);
                if y < actual_overlap {
                    // Linear alpha blending in the overlap zone
                    let alpha = y as f32 / actual_overlap as f32;
                    let top_pixel = canvas.get_pixel(x, canvas_y);
                    let blended_r = ((1.0 - alpha) * top_pixel[0] as f32
                        + alpha * bottom_pixel[0] as f32) as u8;
                    let blended_g = ((1.0 - alpha) * top_pixel[1] as f32
                        + alpha * bottom_pixel[1] as f32) as u8;
                    let blended_b = ((1.0 - alpha) * top_pixel[2] as f32
                        + alpha * bottom_pixel[2] as f32) as u8;
                    canvas.put_pixel(x, canvas_y, Rgba([blended_r, blended_g, blended_b, 255]));
                } else {
                    canvas.put_pixel(x, canvas_y, *bottom_pixel);
                }
            }
        }

        Ok(canvas)
    }
}
