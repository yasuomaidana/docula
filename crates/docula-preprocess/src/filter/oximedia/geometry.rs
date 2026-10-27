//! Lens distortion correction, perspective dewarping, and motion deblur filters.

use crate::error::PreprocessError;
use crate::filter::ImageFilter;
use cv_mosaic::auto_dewarp_dominant_quad;
use image::RgbaImage;
use oximedia_cv::lens_distortion::{CameraIntrinsics, DistortionCoeffs, LensDistortionCorrector};
use oximedia_cv::motion_blur::{DeblurMethod, MotionBlurRemover};

/// Lens distortion correction filter utilizing `oximedia_cv::lens_distortion`.
#[derive(Debug, Clone, Copy)]
pub struct OxiMediaLensUndistortFilter {
    pub fov_degrees: f64,
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
#[derive(Debug, Clone)]
pub struct OxiMediaPerspectiveDewarpFilter {
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
        auto_dewarp_dominant_quad(img, self.min_area_fraction).map_err(|e| {
            PreprocessError::FilterExecution {
                filter: self.name(),
                reason: e.to_string(),
            }
        })
    }
}

/// Motion blur reduction and deconvolution filter utilizing `oximedia_cv::motion_blur`.
#[derive(Debug, Clone, Copy)]
pub struct OxiMediaMotionDeblurFilter {
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
