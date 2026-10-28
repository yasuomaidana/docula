//! CLAHE (Contrast Limited Adaptive Histogram Equalization) and illumination normalization.

use crate::error::PreprocessError;
use crate::filter::ImageFilter;
use image::RgbaImage;
use oximedia_cv::image::filter::{BoxBlur, ImageFilter as OxiCvFilter};
use oximedia_cv::image::histogram::{ClaheConfig, adaptive_histogram_equalization};
use oxipix::mosaic::map_rgb_channels;

/// CLAHE and illumination normalization filter.
#[derive(Debug, Clone)]
pub struct OxiMediaClaheFilter {
    pub clahe_config: ClaheConfig,
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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_tiles(mut self, tiles_x: u32, tiles_y: u32) -> Self {
        self.clahe_config.tiles_x = tiles_x.max(1);
        self.clahe_config.tiles_y = tiles_y.max(1);
        self
    }

    #[must_use]
    pub fn with_clip_limit(mut self, clip_limit: f64) -> Self {
        self.clahe_config.clip_limit = clip_limit.max(1.0);
        self
    }

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

        if width < 32 || height < 32 {
            return Ok(());
        }

        let mut active_config = self.clahe_config;
        active_config.tiles_x = (width / 16).clamp(1, self.clahe_config.tiles_x);
        active_config.tiles_y = (height / 16).clamp(1, self.clahe_config.tiles_y);

        if let Some(radius) = self.background_division_radius {
            let blur = BoxBlur::new(2 * radius + 1);
            map_rgb_channels(img, |_c, chan, _w, _h| {
                if let Ok(bg_estimate) = blur.apply(chan, width, height) {
                    chan.iter()
                        .zip(bg_estimate.iter())
                        .map(|(&fg, &bg)| {
                            let fg_f = f64::from(fg);
                            let bg_f = f64::from(bg).max(1.0);
                            ((fg_f / bg_f) * 220.0).clamp(0.0, 255.0) as u8
                        })
                        .collect()
                } else {
                    chan.to_vec()
                }
            });
        }

        let mut clahe_err = None;
        map_rgb_channels(
            img,
            |_c, chan, _w, _h| match adaptive_histogram_equalization(
                chan,
                width,
                height,
                &active_config,
            ) {
                Ok(equalized) => equalized,
                Err(e) => {
                    clahe_err = Some(e.to_string());
                    chan.to_vec()
                }
            },
        );

        if let Some(err) = clahe_err {
            return Err(PreprocessError::FilterExecution {
                filter: self.name(),
                reason: err,
            });
        }

        Ok(())
    }
}
