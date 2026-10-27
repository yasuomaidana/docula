//! # Vectorized Region Extraction
//!
//! Sub-image visual region representation supporting SVG vector extraction,
//! Moore-Neighbor 8-connected contour tracing, and polygon simplification.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use docula_core::{BoundingBox, CropFormat, ImageCrop};
use image::RgbaImage;
use cv_vectorize::{
    SvgVectorPath, VectorizeConfig, VectorizedGraphic, vectorize_rgba_to_svg,
};

use crate::buffer::encode_rgba_image;
use crate::error::PreprocessError;

/// Extracted visual region of a document page.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualRegion {
    /// In-memory RGBA sub-image representing the visual region.
    pub image: RgbaImage,
    /// Normalized coordinates $[0.0, 1.0]$ on the original parent page.
    pub bbox: BoundingBox,
}

impl VisualRegion {
    /// Creates a new `VisualRegion`.
    #[must_use]
    pub fn new(image: RgbaImage, bbox: BoundingBox) -> Self {
        Self { image, bbox }
    }

    /// Pixel width of this sub-region.
    #[inline]
    #[must_use]
    pub fn width(&self) -> u32 {
        self.image.width()
    }

    /// Pixel height of this sub-region.
    #[inline]
    #[must_use]
    pub fn height(&self) -> u32 {
        self.image.height()
    }

    /// Serializes this visual region into an [`ImageCrop`] value object.
    pub fn to_image_crop(
        &self,
        page_index: usize,
        format: CropFormat,
    ) -> Result<ImageCrop, PreprocessError> {
        let width = self.width();
        let height = self.height();
        let encoded_bytes = encode_rgba_image(&self.image, format)?;
        let encoded_data = BASE64_STANDARD.encode(&encoded_bytes);

        let crop = ImageCrop::new(
            page_index,
            self.bbox,
            width,
            height,
            encoded_data,
            format.mime_type(),
        )?;

        Ok(crop)
    }

    /// Vectorizes this raster visual region into an SVG graphic representation.
    #[must_use]
    pub fn to_vector_graphic(&self) -> VectorizedRegion {
        let config = VectorizeConfig {
            grid_step: 4.0,
            rdp_epsilon: 2.0,
        };
        let graphic = vectorize_rgba_to_svg(&self.image, config);
        VectorizedRegion {
            bbox: self.bbox,
            width: graphic.width,
            height: graphic.height,
            paths: graphic.paths,
        }
    }
}

/// Vectorized graphic representation of a document sub-region containing SVG path contours.
#[derive(Clone, Debug, PartialEq)]
pub struct VectorizedRegion {
    /// Parent page normalized bounding box.
    pub bbox: BoundingBox,
    /// Raster pixel width of the region.
    pub width: u32,
    /// Raster pixel height of the region.
    pub height: u32,
    /// Traced and simplified contour paths.
    pub paths: Vec<SvgVectorPath>,
}

impl VectorizedRegion {
    /// Generates standalone SVG XML markup representing the vectorized paths.
    #[must_use]
    pub fn to_svg_string(&self) -> String {
        let graphic = VectorizedGraphic {
            width: self.width,
            height: self.height,
            paths: self.paths.clone(),
        };
        graphic.to_svg_string()
    }
}
