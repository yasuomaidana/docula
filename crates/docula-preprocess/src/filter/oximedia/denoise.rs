//! Morphological denoising and impulsive noise reduction.

use crate::error::PreprocessError;
use crate::filter::ImageFilter;
use image::RgbaImage;
use oximedia_cv::image::filter::{ImageFilter as OxiCvFilter, MedianFilter};
use oximedia_cv::morphology::{StructuringElement, StructuringElementShape, closing, opening};
use oxipix::mosaic::map_rgb_channels;

/// Structuring element shape for mathematical morphology operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MorphologyShape {
    #[default]
    Rectangle,
    Cross,
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
    Opening,
    Closing,
    #[default]
    FullRestoration,
}

/// Configuration for morphological noise reduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MorphologyDenoiseConfig {
    pub op: MorphologyOp,
    pub shape: MorphologyShape,
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

/// Denoise mode selection for [`OxiMediaDenoiseFilter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenoiseMode {
    PepperRemoval { radius: usize },
    SaltRemoval { radius: usize },
    SaltAndPepper { radius: usize },
    FullRestoration { radius: usize },
}

/// Dedicated document noise reduction and restoration filter utilizing OxiMedia CV morphology.
#[derive(Debug, Clone)]
pub struct OxiMediaDenoiseFilter {
    pub mode: DenoiseMode,
    pub shape: MorphologyShape,
}

impl Default for OxiMediaDenoiseFilter {
    fn default() -> Self {
        Self::salt_and_pepper(1)
    }
}

impl OxiMediaDenoiseFilter {
    #[must_use]
    pub fn new(mode: DenoiseMode) -> Self {
        Self {
            mode,
            shape: MorphologyShape::Rectangle,
        }
    }

    #[must_use]
    pub fn salt_and_pepper(radius: usize) -> Self {
        Self::new(DenoiseMode::SaltAndPepper {
            radius: radius.max(1),
        })
    }

    #[must_use]
    pub fn pepper_removal(radius: usize) -> Self {
        Self::new(DenoiseMode::PepperRemoval {
            radius: radius.max(1),
        })
    }

    #[must_use]
    pub fn salt_removal(radius: usize) -> Self {
        Self::new(DenoiseMode::SaltRemoval {
            radius: radius.max(1),
        })
    }

    #[must_use]
    pub fn full_restoration(radius: usize) -> Self {
        Self::new(DenoiseMode::FullRestoration {
            radius: radius.max(1),
        })
    }

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
pub fn apply_morphology(
    img: &mut RgbaImage,
    cfg: MorphologyDenoiseConfig,
) -> Result<(), PreprocessError> {
    if cfg.op == MorphologyOp::FullRestoration {
        apply_median_3x3(img)?;
    }

    let k_size = 2 * cfg.radius + 1;
    let se_shape: StructuringElementShape = cfg.shape.into();
    let se = StructuringElement::from_shape(se_shape, k_size, k_size);

    map_rgb_channels(img, |_c, chan, w, h| {
        let float_buf: Vec<f64> = chan.iter().map(|&b| f64::from(b)).collect();
        let processed = match cfg.op {
            MorphologyOp::Opening => opening(&float_buf, w, h, &se),
            MorphologyOp::Closing => closing(&float_buf, w, h, &se),
            MorphologyOp::FullRestoration => opening(&float_buf, w, h, &se),
        };
        processed
            .into_iter()
            .map(|v| v.round().clamp(0.0, 255.0) as u8)
            .collect()
    });

    Ok(())
}

/// Applies a 3x3 median filter across RGB channels using `oximedia-cv`.
pub fn apply_median_3x3(img: &mut RgbaImage) -> Result<(), PreprocessError> {
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
