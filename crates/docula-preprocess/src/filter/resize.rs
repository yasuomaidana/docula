use super::ImageFilter;
use crate::error::PreprocessError;
use image::RgbaImage;
use oximedia_cv::image::resize::{ResizeConfig, ResizeMethod, resize_image};

/// Scaling strategy for [`SimdResizeFilter`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResizeMode {
    /// Exact explicit width and height (may alter aspect ratio).
    Exact { width: u32, height: u32 },
    /// Constrain within a bounding box, preserving aspect ratio.
    Fit { max_width: u32, max_height: u32 },
    /// Target a specific width, calculating height to preserve aspect ratio.
    FitWidth(u32),
    /// Target a specific height, calculating width to preserve aspect ratio.
    FitHeight(u32),
    /// Multiplicative scale factors.
    Scale { factor_x: f32, factor_y: f32 },
}

/// SIMD and GPU-accelerated image rescaler using KitaSan `oximedia-cv`.
///
/// Implements [`ImageFilter`] for high-throughput scaling without CPU bottlenecks.
#[derive(Debug, Clone, Copy)]
pub struct SimdResizeFilter {
    mode: ResizeMode,
}

impl SimdResizeFilter {
    /// Constructs a resizer with the given [`ResizeMode`].
    #[must_use]
    pub fn new(mode: ResizeMode) -> Self {
        Self { mode }
    }

    /// Convenience constructor for exact target width and height.
    #[must_use]
    pub fn exact(width: u32, height: u32) -> Self {
        Self::new(ResizeMode::Exact { width, height })
    }

    /// Convenience constructor for bounding box aspect-ratio preserving fit.
    #[must_use]
    pub fn fit(max_width: u32, max_height: u32) -> Self {
        Self::new(ResizeMode::Fit {
            max_width,
            max_height,
        })
    }

    /// Convenience constructor for fixing width and scaling height proportionally.
    #[must_use]
    pub fn fit_width(width: u32) -> Self {
        Self::new(ResizeMode::FitWidth(width))
    }

    /// Convenience constructor for fixing height and scaling width proportionally.
    #[must_use]
    pub fn fit_height(height: u32) -> Self {
        Self::new(ResizeMode::FitHeight(height))
    }

    /// Computes destination dimensions `(target_width, target_height)` based on input dimensions.
    ///
    /// # Errors
    /// Returns [`PreprocessError::InvalidDimensions`] if target dimensions evaluate to zero.
    pub fn compute_target_dimensions(
        &self,
        src_w: u32,
        src_h: u32,
    ) -> Result<(u32, u32), PreprocessError> {
        if src_w == 0 || src_h == 0 {
            return Err(PreprocessError::InvalidDimensions {
                width: src_w,
                height: src_h,
                reason: "source dimensions must be positive".to_string(),
            });
        }

        let (dst_w, dst_h) = match self.mode {
            ResizeMode::Exact { width, height } => (width, height),
            ResizeMode::Fit {
                max_width,
                max_height,
            } => {
                if max_width == 0 || max_height == 0 {
                    return Err(PreprocessError::InvalidDimensions {
                        width: max_width,
                        height: max_height,
                        reason: "fit bounding box must be positive".to_string(),
                    });
                }
                let ratio_w = max_width as f64 / src_w as f64;
                let ratio_h = max_height as f64 / src_h as f64;
                let scale = ratio_w.min(ratio_h);
                let w = (src_w as f64 * scale).round() as u32;
                let h = (src_h as f64 * scale).round() as u32;
                (w.max(1), h.max(1))
            }
            ResizeMode::FitWidth(target_w) => {
                if target_w == 0 {
                    return Err(PreprocessError::InvalidDimensions {
                        width: 0,
                        height: src_h,
                        reason: "target width must be positive".to_string(),
                    });
                }
                let h = ((u64::from(src_h) * u64::from(target_w) + u64::from(src_w) / 2)
                    / u64::from(src_w)) as u32;
                (target_w, h.max(1))
            }
            ResizeMode::FitHeight(target_h) => {
                if target_h == 0 {
                    return Err(PreprocessError::InvalidDimensions {
                        width: src_w,
                        height: 0,
                        reason: "target height must be positive".to_string(),
                    });
                }
                let w = ((u64::from(src_w) * u64::from(target_h) + u64::from(src_h) / 2)
                    / u64::from(src_h)) as u32;
                (w.max(1), target_h)
            }
            ResizeMode::Scale { factor_x, factor_y } => {
                if factor_x <= 0.0 || factor_y <= 0.0 || factor_x.is_nan() || factor_y.is_nan() {
                    return Err(PreprocessError::InvalidDimensions {
                        width: 0,
                        height: 0,
                        reason: format!(
                            "scale factors must be positive and non-NaN ({factor_x}, {factor_y})"
                        ),
                    });
                }
                let w = (src_w as f32 * factor_x).round() as u32;
                let h = (src_h as f32 * factor_y).round() as u32;
                (w.max(1), h.max(1))
            }
        };

        if dst_w == 0 || dst_h == 0 {
            return Err(PreprocessError::InvalidDimensions {
                width: dst_w,
                height: dst_h,
                reason: "computed destination dimensions must be positive".to_string(),
            });
        }

        Ok((dst_w, dst_h))
    }
}

impl ImageFilter for SimdResizeFilter {
    fn name(&self) -> &'static str {
        "SimdResizeFilter"
    }

    fn apply(&self, img: &mut RgbaImage) -> Result<(), PreprocessError> {
        let (src_w, src_h) = img.dimensions();
        let (dst_w, dst_h) = self.compute_target_dimensions(src_w, src_h)?;

        // If target size matches source, no-op
        if src_w == dst_w && src_h == dst_h {
            return Ok(());
        }

        let config = ResizeConfig::new(dst_w, dst_h, ResizeMethod::Bilinear, 4);
        let resized = resize_image(img.as_raw(), src_w, src_h, &config).map_err(|e| {
            PreprocessError::ResizeFailure {
                reason: e.to_string(),
            }
        })?;

        let dst_image = RgbaImage::from_raw(dst_w, dst_h, resized).ok_or_else(|| {
            PreprocessError::ResizeFailure {
                reason: "failed to construct destination RGBA image from resized bytes".to_string(),
            }
        })?;

        *img = dst_image;
        Ok(())
    }
}
