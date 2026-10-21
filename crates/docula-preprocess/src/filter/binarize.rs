use super::ImageFilter;
use crate::error::PreprocessError;
use image::{GrayImage, Luma, Rgba, RgbaImage};

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

/// Computes the Otsu optimal bimodal threshold level for a grayscale buffer.
#[must_use]
pub fn compute_otsu_level(gray: &GrayImage) -> u8 {
    let mut histogram = [0u32; 256];
    for p in gray.pixels() {
        histogram[p[0] as usize] += 1;
    }

    let total = (gray.width() as usize * gray.height() as usize) as f64;
    if total == 0.0 {
        return 128;
    }

    let mut sum_all = 0.0;
    for (i, &count) in histogram.iter().enumerate() {
        sum_all += (i as f64) * (count as f64);
    }

    let mut sum_b = 0.0;
    let mut weight_b = 0.0;
    let mut max_variance = 0.0;
    let mut threshold = 128u8;

    for (t, &count) in histogram.iter().enumerate() {
        weight_b += count as f64;
        if weight_b == 0.0 {
            continue;
        }
        let weight_f = total - weight_b;
        if weight_f == 0.0 {
            break;
        }

        sum_b += (t as f64) * (count as f64);
        let mean_b = sum_b / weight_b;
        let mean_f = (sum_all - sum_b) / weight_f;

        let variance = weight_b * weight_f * (mean_b - mean_f) * (mean_b - mean_f);
        if variance > max_variance {
            max_variance = variance;
            threshold = t as u8;
        }
    }

    threshold
}

/// Applies local mean adaptive thresholding using a 2D integral image.
fn compute_adaptive_threshold(gray: &GrayImage, block_radius: u32) -> GrayImage {
    let width = gray.width();
    let height = gray.height();
    let w = width as usize;
    let h = height as usize;
    let stride = w + 1;

    let mut integral = vec![0u32; stride * (h + 1)];
    for y in 0..h {
        let mut row_sum = 0u32;
        for x in 0..w {
            row_sum += gray.get_pixel(x as u32, y as u32)[0] as u32;
            integral[(y + 1) * stride + (x + 1)] = integral[y * stride + (x + 1)] + row_sum;
        }
    }

    let mut out = GrayImage::new(width, height);
    for y in 0..height {
        let y_low = (y as i32 - block_radius as i32).max(0) as usize;
        let y_high = (y + block_radius).min(height - 1) as usize;

        for x in 0..width {
            let x_low = (x as i32 - block_radius as i32).max(0) as usize;
            let x_high = (x + block_radius).min(width - 1) as usize;

            let x1 = x_low;
            let y1 = y_low;
            let x2 = x_high + 1;
            let y2 = y_high + 1;

            let sum = integral[y2 * stride + x2] + integral[y1 * stride + x1]
                - integral[y1 * stride + x2]
                - integral[y2 * stride + x1];
            let count = (x2 - x1) * (y2 - y1);
            let mean = (sum / count as u32) as u8;

            let val = gray.get_pixel(x, y)[0];
            let bin_val = if val >= mean { 255 } else { 0 };
            out.put_pixel(x, y, Luma([bin_val]));
        }
    }

    out
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
                compute_adaptive_threshold(&gray, block_radius)
            }
            BinarizeMethod::Otsu => {
                let level = compute_otsu_level(&gray);
                let mut out = GrayImage::new(width, height);
                for y in 0..height {
                    for x in 0..width {
                        let val = gray.get_pixel(x, y)[0];
                        let bin = if val > level { 255 } else { 0 };
                        out.put_pixel(x, y, Luma([bin]));
                    }
                }
                out
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
