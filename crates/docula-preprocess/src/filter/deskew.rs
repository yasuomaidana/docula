use super::ImageFilter;
use crate::error::PreprocessError;
use image::{GrayImage, Luma, Rgba, RgbaImage};
use imageproc::contrast::otsu_level;
use imageproc::geometric_transformations::{Interpolation, rotate_about_center};
use imageproc::hough::{LineDetectionOptions, detect_lines};
use std::collections::HashMap;

/// Hough transform based image deskew filter.
///
/// Detects the dominant baseline orientation of document text lines and structural rulings
/// in the range $[-45^\circ, +45^\circ]$, then applies an affine rotational correction
/// with $\pm 0.5^\circ$ (typically $\pm 0.1^\circ$) precision.
#[derive(Debug, Clone)]
pub struct HoughDeskewFilter {
    /// Minimum vote threshold in Hough accumulator. If `None`, dynamically sized.
    pub vote_threshold: Option<u32>,
    /// Non-maxima suppression radius in the Hough accumulator.
    pub suppression_radius: u32,
    /// Maximum skew angle in degrees to detect and correct (default 45.0).
    pub max_angle_degrees: f32,
    /// Minimum detected skew magnitude in degrees before applying rotation.
    pub min_angle_threshold: f32,
}

impl Default for HoughDeskewFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl HoughDeskewFilter {
    /// Creates a new `HoughDeskewFilter` with default document settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            vote_threshold: None,
            suppression_radius: 8,
            max_angle_degrees: 45.0,
            min_angle_threshold: 0.1,
        }
    }

    /// Sets an explicit vote threshold for line detection.
    #[must_use]
    pub fn with_vote_threshold(mut self, threshold: u32) -> Self {
        self.vote_threshold = Some(threshold);
        self
    }

    /// Sets the suppression radius for line non-maxima suppression.
    #[must_use]
    pub fn with_suppression_radius(mut self, radius: u32) -> Self {
        self.suppression_radius = radius;
        self
    }

    /// Sets the maximum allowable skew angle in degrees (clamped to $[0.0, 45.0]$).
    #[must_use]
    pub fn with_max_angle(mut self, max_degrees: f32) -> Self {
        self.max_angle_degrees = max_degrees.clamp(0.0, 45.0);
        self
    }

    /// Sets the minimum angle threshold in degrees before triggering rotation.
    #[must_use]
    pub fn with_min_threshold(mut self, min_degrees: f32) -> Self {
        self.min_angle_threshold = min_degrees.max(0.0);
        self
    }

    /// Estimates the dominant skew angle in degrees of the given image.
    ///
    /// Returns `Ok(Some(angle_degrees))` where positive indicates clockwise tilt,
    /// or `Ok(None)` if no dominant linear orientation was identified.
    ///
    /// # Errors
    /// Returns [`PreprocessError::InvalidDimensions`] if the image is empty.
    pub fn detect_skew_angle(&self, img: &RgbaImage) -> Result<Option<f32>, PreprocessError> {
        let (width, height) = img.dimensions();
        if width == 0 || height == 0 {
            return Err(PreprocessError::InvalidDimensions {
                width,
                height,
                reason: "cannot detect skew on empty image".to_string(),
            });
        }

        // Convert to grayscale
        let mut gray = GrayImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let p = img.get_pixel(x, y);
                let luma =
                    (u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000;
                gray.put_pixel(x, y, Luma([luma as u8]));
            }
        }

        // Extract foreground text/ruling pixels via Otsu thresholding
        let level = otsu_level(&gray);
        let dark_count = gray.pixels().filter(|p| p[0] < level).count();
        let light_count = (width as usize * height as usize) - dark_count;
        let text_is_dark = dark_count <= light_count;

        let mut bin = GrayImage::new(width, height);
        let mut foreground_points = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let val = gray.get_pixel(x, y)[0];
                let is_fg = if text_is_dark {
                    val < level
                } else {
                    val > level
                };
                if is_fg {
                    bin.put_pixel(x, y, Luma([255]));
                    foreground_points.push((x as f32, y as f32));
                }
            }
        }

        if foreground_points.len() < 20 {
            return Ok(None);
        }

        // Calculate Hough vote threshold
        let vote_thresh = self
            .vote_threshold
            .unwrap_or_else(|| (width.min(height) / 8).clamp(30, 150));

        let options = LineDetectionOptions {
            vote_threshold: vote_thresh,
            suppression_radius: self.suppression_radius,
        };

        let mut lines = detect_lines(&bin, options);
        // Fallback: if no lines found and dynamic threshold was used, try lower threshold
        if lines.is_empty() && self.vote_threshold.is_none() && vote_thresh > 30 {
            let fallback_opts = LineDetectionOptions {
                vote_threshold: 30,
                suppression_radius: self.suppression_radius,
            };
            lines = detect_lines(&bin, fallback_opts);
        }

        if lines.is_empty() {
            return Ok(None);
        }

        // Filter lines near horizontal (normal vector in [45°, 135°])
        // Horizontal line normal is 90°. A line tilted by theta has normal (90° + theta).
        let min_hough_angle = 90.0 - self.max_angle_degrees;
        let max_hough_angle = 90.0 + self.max_angle_degrees;

        let mut candidate_skews: Vec<f32> = lines
            .iter()
            .map(|l| l.angle_in_degrees as f32)
            .filter(|&a| a >= min_hough_angle && a <= max_hough_angle)
            .map(|a| a - 90.0)
            .collect();

        if candidate_skews.is_empty() {
            return Ok(None);
        }

        // Compute median candidate angle
        candidate_skews.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let coarse_angle = candidate_skews[candidate_skews.len() / 2];

        // Sub-degree refinement around coarse angle within ±1.0° with 0.1° resolution
        let refined_angle = self.refine_subdegree_angle(&foreground_points, coarse_angle);

        Ok(Some(refined_angle))
    }

    /// Performs fine-grained Hough projection around `coarse_angle` to achieve sub-degree accuracy.
    fn refine_subdegree_angle(&self, points: &[(f32, f32)], coarse_angle: f32) -> f32 {
        // Subsample up to 4000 points if image is very dense for sub-millisecond execution
        let step = (points.len() / 4000).max(1);
        let sampled_points: Vec<(f32, f32)> = points.iter().step_by(step).copied().collect();

        let mut best_angle = coarse_angle;
        let mut best_peak_votes = 0u32;

        // Search in [coarse_angle - 1.0, coarse_angle + 1.0] at 0.1° increments (21 steps)
        let mut delta_step = -10;
        while delta_step <= 10 {
            let cand_skew = coarse_angle + (delta_step as f32 * 0.1);
            let normal_rad = (90.0 + cand_skew).to_radians();
            let (sin_t, cos_t) = normal_rad.sin_cos();

            // Accumulate projection onto normal: rho = x * cos_t + y * sin_t
            let mut histogram: HashMap<i32, u32> = HashMap::new();
            for &(x, y) in &sampled_points {
                let rho = (x * cos_t + y * sin_t).round() as i32;
                *histogram.entry(rho).or_insert(0) += 1;
            }

            let peak_votes = histogram.values().copied().max().unwrap_or(0);
            if peak_votes > best_peak_votes {
                best_peak_votes = peak_votes;
                best_angle = cand_skew;
            }

            delta_step += 1;
        }

        best_angle
    }
}

impl ImageFilter for HoughDeskewFilter {
    fn name(&self) -> &'static str {
        "HoughDeskewFilter"
    }

    fn apply(&self, img: &mut RgbaImage) -> Result<(), PreprocessError> {
        let skew_opt = self.detect_skew_angle(img)?;

        if let Some(skew) = skew_opt
            && skew.abs() >= self.min_angle_threshold
        {
            // To counteract clockwise tilt (+skew), rotate counter-clockwise (-skew)
            let correction_radians = (-skew).to_radians();
            let white_bg = Rgba([255, 255, 255, 255]);
            let rotated =
                rotate_about_center(img, correction_radians, Interpolation::Bilinear, white_bg);
            *img = rotated;
        }

        Ok(())
    }
}
