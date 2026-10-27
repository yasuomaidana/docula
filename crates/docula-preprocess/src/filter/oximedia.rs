//! OxiMedia pure-Rust computer vision integration for docula preprocessing.
//!
//! Wraps the KitaSan `oximedia`, `oximedia-core`, and `oximedia-cv` framework
//! to provide sovereign, patent-free image processing, filtering, and computer vision
//! transformations without any OpenCV/FFmpeg dynamic C/C++ dependencies.

use crate::error::PreprocessError;
use crate::filter::ImageFilter;
use image::{Rgba, RgbaImage};
use oximedia_core::pixel_format::PixelFormat;
use oximedia_cv::contour::{ContourApprox, ContourConfig, ContourFinder};
use oximedia_cv::enhance::sharpening::{UnsharpMaskParams, laplacian_sharpen, unsharp_mask};
use oximedia_cv::image::filter::{BoxBlur, ImageFilter as OxiCvFilter, MedianFilter};
use oximedia_cv::image::histogram::{ClaheConfig, adaptive_histogram_equalization};
use oximedia_cv::lens_distortion::{CameraIntrinsics, DistortionCoeffs, LensDistortionCorrector};
use oximedia_cv::morphology::{StructuringElement, StructuringElementShape, closing, opening};
use oximedia_cv::motion_blur::{DeblurMethod, MotionBlurRemover};
use oximedia_cv::panorama_stitch::{
    PanoFormat, PanoramaConfig as OxiPanoConfig, PanoramaStitcher as OxiPanoStitcher,
    StitchFrame as OxiStitchFrame,
};
use oximedia_cv::transform::perspective::{quad_to_rect, warp_perspective};

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

/// CLAHE (Contrast Limited Adaptive Histogram Equalization) and illumination normalization filter.
///
/// Flattens uneven lighting gradients, phone camera shadows, and paper fold creases while
/// enhancing local document stroke contrast without blowing out highlights or amplifying noise.
#[derive(Debug, Clone)]
pub struct OxiMediaClaheFilter {
    /// CLAHE configuration parameters.
    pub clahe_config: ClaheConfig,
    /// Optional box blur kernel radius for background division illumination leveling.
    pub background_division_radius: Option<usize>,
}

impl Default for OxiMediaClaheFilter {
    fn default() -> Self {
        Self {
            clahe_config: ClaheConfig {
                tiles_x: 8,
                tiles_y: 8,
                clip_limit: 2.0,
            },
            background_division_radius: Some(15),
        }
    }
}

impl OxiMediaClaheFilter {
    /// Creates a new CLAHE filter with default 8x8 tiles and clip limit 2.0.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the grid tile divisions (e.g. 4x4 or 8x8).
    #[must_use]
    pub fn with_tiles(mut self, tiles_x: u32, tiles_y: u32) -> Self {
        self.clahe_config.tiles_x = tiles_x.max(1);
        self.clahe_config.tiles_y = tiles_y.max(1);
        self
    }

    /// Sets the contrast clipping limit (1.0 = standard, 2.0-3.0 = aggressive).
    #[must_use]
    pub fn with_clip_limit(mut self, clip_limit: f64) -> Self {
        self.clahe_config.clip_limit = clip_limit.max(1.0);
        self
    }

    /// Enables or disables background division illumination leveling.
    #[must_use]
    pub fn with_background_division(mut self, radius: Option<usize>) -> Self {
        self.background_division_radius = radius;
        self
    }
}

impl ImageFilter for OxiMediaClaheFilter {
    fn name(&self) -> &'static str {
        "OxiMediaClaheFilter"
    }

    fn apply(&self, img: &mut RgbaImage) -> Result<(), PreprocessError> {
        let (width, height) = img.dimensions();
        if width == 0 || height == 0 {
            return Err(PreprocessError::InvalidDimensions {
                width,
                height,
                reason: "Dimensions must be non-zero for OxiMedia CLAHE".to_string(),
            });
        }

        let w = width as usize;
        let h = height as usize;

        // 1. Optional background illumination division using fast box blur
        if let Some(radius) = self.background_division_radius {
            let blur = BoxBlur::new(2 * radius + 1);
            for c in 0..3 {
                let mut channel_buf = Vec::with_capacity(w * h);
                for y in 0..height {
                    for x in 0..width {
                        channel_buf.push(img.get_pixel(x, y)[c]);
                    }
                }

                if let Ok(bg_estimate) = blur.apply(&channel_buf, width, height) {
                    for y in 0..height {
                        for x in 0..width {
                            let idx = (y as usize) * w + (x as usize);
                            let fg = f64::from(img.get_pixel(x, y)[c]);
                            let bg = f64::from(bg_estimate[idx]).max(1.0);
                            let normalized = ((fg / bg) * 220.0).clamp(0.0, 255.0) as u8;
                            img.get_pixel_mut(x, y)[c] = normalized;
                        }
                    }
                }
            }
        }

        // 2. Apply CLAHE per RGB channel using oximedia-cv
        for c in 0..3 {
            let mut channel_buf = Vec::with_capacity(w * h);
            for y in 0..height {
                for x in 0..width {
                    channel_buf.push(img.get_pixel(x, y)[c]);
                }
            }

            let equalized =
                adaptive_histogram_equalization(&channel_buf, width, height, &self.clahe_config)
                    .map_err(|e| PreprocessError::FilterExecution {
                        filter: self.name(),
                        reason: e.to_string(),
                    })?;

            for y in 0..height {
                for x in 0..width {
                    let idx = (y as usize) * w + (x as usize);
                    img.get_pixel_mut(x, y)[c] = equalized[idx];
                }
            }
        }

        Ok(())
    }
}

/// Lens distortion correction filter utilizing `oximedia_cv::lens_distortion`.
///
/// Corrects mobile wide-angle camera radial (barrel/pincushion) and tangential distortions
/// before document stitching or OCR so straight ruler lines and table grids remain straight.
#[derive(Debug, Clone, Copy)]
pub struct OxiMediaLensUndistortFilter {
    /// Camera horizontal field of view in degrees.
    pub fov_degrees: f64,
    /// Radial barrel distortion coefficient (e.g. 0.1 to 0.35).
    pub barrel_k1: f64,
}

impl Default for OxiMediaLensUndistortFilter {
    fn default() -> Self {
        Self {
            fov_degrees: 65.0,
            barrel_k1: 0.15,
        }
    }
}

impl OxiMediaLensUndistortFilter {
    /// Creates a lens undistortion filter with given FOV and barrel coefficient.
    #[must_use]
    pub fn new(fov_degrees: f64, barrel_k1: f64) -> Self {
        Self {
            fov_degrees: fov_degrees.clamp(10.0, 120.0),
            barrel_k1,
        }
    }
}

impl ImageFilter for OxiMediaLensUndistortFilter {
    fn name(&self) -> &'static str {
        "OxiMediaLensUndistortFilter"
    }

    fn apply(&self, img: &mut RgbaImage) -> Result<(), PreprocessError> {
        let (width, height) = img.dimensions();
        if width == 0 || height == 0 {
            return Err(PreprocessError::InvalidDimensions {
                width,
                height,
                reason: "Dimensions must be non-zero for lens undistortion".to_string(),
            });
        }

        let w = width as usize;
        let h = height as usize;
        let intrinsics = CameraIntrinsics::from_fov(w, h, self.fov_degrees.to_radians());
        let coeffs = DistortionCoeffs::barrel(self.barrel_k1);
        let corrector = LensDistortionCorrector::new(intrinsics, coeffs);

        let corrected_rgba = corrector.correct_rgba(img.as_raw(), w, h);
        if corrected_rgba.len() == img.as_raw().len() {
            img.copy_from_slice(&corrected_rgba);
        }

        Ok(())
    }
}

/// Automatic perspective rectification and sheet dewarping filter.
///
/// Uses `oximedia_cv::contour::ContourFinder` to detect the dominant 4-corner document boundary
/// and `oximedia_cv::transform::perspective` to unwarp the tilted document page into an orthogonal rectangle.
#[derive(Debug, Clone)]
pub struct OxiMediaPerspectiveDewarpFilter {
    /// Minimum contour area fraction of image required to trigger dewarping (default 0.20 = 20%).
    pub min_area_fraction: f32,
}

impl Default for OxiMediaPerspectiveDewarpFilter {
    fn default() -> Self {
        Self {
            min_area_fraction: 0.20,
        }
    }
}

impl OxiMediaPerspectiveDewarpFilter {
    /// Creates a new perspective dewarping filter with configurable minimum area threshold.
    #[must_use]
    pub fn new(min_area_fraction: f32) -> Self {
        Self {
            min_area_fraction: min_area_fraction.clamp(0.05, 0.95),
        }
    }
}

impl ImageFilter for OxiMediaPerspectiveDewarpFilter {
    fn name(&self) -> &'static str {
        "OxiMediaPerspectiveDewarpFilter"
    }

    fn apply(&self, img: &mut RgbaImage) -> Result<(), PreprocessError> {
        let (width, height) = img.dimensions();
        if width < 10 || height < 10 {
            return Ok(());
        }

        let total_area = (width as f32) * (height as f32);
        let min_area = total_area * self.min_area_fraction;

        // Convert to grayscale for contour extraction
        let mut gray = Vec::with_capacity((width * height) as usize);
        for pixel in img.pixels() {
            let luma =
                (u32::from(pixel[0]) * 299 + u32::from(pixel[1]) * 587 + u32::from(pixel[2]) * 114)
                    / 1000;
            gray.push(luma as u8);
        }

        let finder = ContourFinder::with_config(ContourConfig {
            approx: ContourApprox::Simple,
            min_area,
            max_area: 0.0,
            retrieve_holes: false,
        });

        let contours = finder.find_contours(&gray, width as usize, height as usize);
        let candidates = finder.filter_by_area(&contours);

        // Find best candidate quadrilateral
        for contour in candidates {
            if contour.points.len() >= 4
                && let Some((min_x, min_y, max_x, max_y)) = contour.bounding_box()
            {
                let w = (max_x - min_x).max(1) as f64;
                let h = (max_y - min_y).max(1) as f64;

                // If already aligned to within 3%, skip dewarp
                if w >= (width as f64 * 0.96) && h >= (height as f64 * 0.96) {
                    return Ok(());
                }

                let quad = [
                    (min_x as f64, min_y as f64),
                    (max_x as f64, min_y as f64),
                    (max_x as f64, max_y as f64),
                    (min_x as f64, max_y as f64),
                ];

                if let Ok(transform) = quad_to_rect(&quad, width as f64, height as f64) {
                    let w_u32 = width;
                    let h_u32 = height;

                    for c in 0..4 {
                        let mut ch_in = Vec::with_capacity((w_u32 * h_u32) as usize);
                        for y in 0..height {
                            for x in 0..width {
                                ch_in.push(img.get_pixel(x, y)[c]);
                            }
                        }
                        if let Ok(warped_ch) =
                            warp_perspective(&ch_in, w_u32, h_u32, &transform, w_u32, h_u32)
                        {
                            for y in 0..height {
                                for x in 0..width {
                                    let idx = (y as usize) * (width as usize) + (x as usize);
                                    img.get_pixel_mut(x, y)[c] = warped_ch[idx];
                                }
                            }
                        }
                    }
                    return Ok(());
                }
            }
        }

        Ok(())
    }
}

/// Motion blur reduction and deconvolution filter utilizing `oximedia_cv::motion_blur`.
#[derive(Debug, Clone, Copy)]
pub struct OxiMediaMotionDeblurFilter {
    /// Motion blur removal method.
    pub method: DeblurMethod,
}

impl Default for OxiMediaMotionDeblurFilter {
    fn default() -> Self {
        Self {
            method: DeblurMethod::Blind,
        }
    }
}

impl OxiMediaMotionDeblurFilter {
    /// Creates a new motion deblur filter with specified deconvolution method.
    #[must_use]
    pub fn new(method: DeblurMethod) -> Self {
        Self { method }
    }
}

impl ImageFilter for OxiMediaMotionDeblurFilter {
    fn name(&self) -> &'static str {
        "OxiMediaMotionDeblurFilter"
    }

    fn apply(&self, img: &mut RgbaImage) -> Result<(), PreprocessError> {
        let (width, height) = img.dimensions();
        if width < 8 || height < 8 {
            return Ok(());
        }

        let w = width as usize;
        let h = height as usize;
        let mut rgb_buf = Vec::with_capacity(w * h * 3);
        for y in 0..height {
            for x in 0..width {
                let p = img.get_pixel(x, y);
                rgb_buf.push(p[0]);
                rgb_buf.push(p[1]);
                rgb_buf.push(p[2]);
            }
        }

        let remover = MotionBlurRemover::new(self.method);
        if let Ok(deblurred) = remover.remove_blur(&rgb_buf, width, height)
            && deblurred.len() == rgb_buf.len()
        {
            for y in 0..height {
                for x in 0..width {
                    let idx = ((y as usize) * w + (x as usize)) * 3;
                    let p = img.get_pixel_mut(x, y);
                    p[0] = deblurred[idx];
                    p[1] = deblurred[idx + 1];
                    p[2] = deblurred[idx + 2];
                }
            }
        }

        Ok(())
    }
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

    /// Merges two horizontally overlapping document image sections into a unified canvas.
    ///
    /// # Errors
    /// Returns [`PreprocessError`] if buffers are empty.
    pub fn stitch_horizontal(
        left: &RgbaImage,
        right: &RgbaImage,
        overlap_width: u32,
    ) -> Result<RgbaImage, PreprocessError> {
        let (w_left, h_left) = left.dimensions();
        let (w_right, h_right) = right.dimensions();

        if w_left == 0 || h_left == 0 || w_right == 0 || h_right == 0 {
            return Err(PreprocessError::InvalidDimensions {
                width: w_left,
                height: h_left,
                reason: "Image dimensions must be non-zero for horizontal stitching".to_string(),
            });
        }

        let unified_height = h_left.max(h_right);
        let actual_overlap = overlap_width.min(w_left).min(w_right);
        let unified_width = w_left + w_right - actual_overlap;

        let mut canvas =
            RgbaImage::from_pixel(unified_width, unified_height, Rgba([255, 255, 255, 255]));

        // Copy left image
        for y in 0..h_left {
            for x in 0..w_left {
                canvas.put_pixel(x, y, *left.get_pixel(x, y));
            }
        }

        // Blend overlapping seam and append right image
        let seam_start = w_left - actual_overlap;
        for y in 0..h_right {
            for x in 0..w_right {
                let canvas_x = seam_start + x;
                let right_pixel = right.get_pixel(x, y);
                if x < actual_overlap {
                    let alpha = x as f32 / actual_overlap as f32;
                    let left_pixel = canvas.get_pixel(canvas_x, y);
                    let blended_r = ((1.0 - alpha) * left_pixel[0] as f32
                        + alpha * right_pixel[0] as f32) as u8;
                    let blended_g = ((1.0 - alpha) * left_pixel[1] as f32
                        + alpha * right_pixel[1] as f32) as u8;
                    let blended_b = ((1.0 - alpha) * left_pixel[2] as f32
                        + alpha * right_pixel[2] as f32) as u8;
                    canvas.put_pixel(canvas_x, y, Rgba([blended_r, blended_g, blended_b, 255]));
                } else {
                    canvas.put_pixel(canvas_x, y, *right_pixel);
                }
            }
        }

        Ok(canvas)
    }

    /// Automatically detects vertical overlap between two consecutive document slices.
    #[must_use]
    pub fn detect_overlap_vertical(top: &RgbaImage, bottom: &RgbaImage) -> u32 {
        let (w_top, h_top) = top.dimensions();
        let (w_bottom, h_bottom) = bottom.dimensions();
        let max_overlap = (h_top / 2).min(h_bottom / 2).max(10);
        let w = w_top.min(w_bottom);

        let mut best_overlap = max_overlap.min(30);
        let mut min_diff = u64::MAX;

        for cand in (10..max_overlap).step_by(2) {
            let mut diff = 0u64;
            let sample_rows = 5.min(cand);
            for r in 0..sample_rows {
                let y_top = h_top - cand + r;
                let y_bot = r;
                for x in (0..w).step_by(4) {
                    let p1 = top.get_pixel(x, y_top);
                    let p2 = bottom.get_pixel(x, y_bot);
                    let d = (i32::from(p1[0]) - i32::from(p2[0])).unsigned_abs() as u64
                        + (i32::from(p1[1]) - i32::from(p2[1])).unsigned_abs() as u64
                        + (i32::from(p1[2]) - i32::from(p2[2])).unsigned_abs() as u64;
                    diff += d;
                }
            }
            if diff < min_diff {
                min_diff = diff;
                best_overlap = cand;
            }
        }

        best_overlap
    }

    /// Stitches multiple RGBA frames with pixel offsets and linear alpha feathering
    /// using `oximedia_cv::panorama_stitch::PanoramaStitcher`.
    ///
    /// # Errors
    /// Returns [`PreprocessError`] if stitching fails or frames are empty.
    pub fn stitch_frames_feathered(
        frames: &[(&RgbaImage, i64, i64)],
        blend_width_px: u32,
    ) -> Result<RgbaImage, PreprocessError> {
        if frames.is_empty() {
            return Err(PreprocessError::InvalidDimensions {
                width: 0,
                height: 0,
                reason: "No frames provided for feathered stitching".to_string(),
            });
        }

        let stitch_frames: Vec<OxiStitchFrame> = frames
            .iter()
            .map(|(img, ox, oy)| {
                let (w, h) = img.dimensions();
                OxiStitchFrame {
                    image: img.as_raw().clone(),
                    width: w,
                    height: h,
                    offset_x: *ox,
                    offset_y: *oy,
                }
            })
            .collect();

        let config = OxiPanoConfig {
            blend_width_px,
            output_format: PanoFormat::Flat,
        };

        let stitcher = OxiPanoStitcher::new(config);
        let pano =
            stitcher
                .stitch(&stitch_frames)
                .map_err(|e| PreprocessError::FilterExecution {
                    filter: "OxiMediaStitcher",
                    reason: format!("Panorama feather stitch failed: {e}"),
                })?;

        RgbaImage::from_raw(pano.width, pano.height, pano.data).ok_or_else(|| {
            PreprocessError::FilterExecution {
                filter: "OxiMediaStitcher",
                reason: "Failed to construct RgbaImage from panorama buffer".to_string(),
            }
        })
    }
}
