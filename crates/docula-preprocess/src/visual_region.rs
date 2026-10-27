//! Visual region extraction, foreground isolation, and vectorization engine.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use cv_mosaic::rgb_to_luma_u8;
use cv_vectorize::{VectorizeConfig, VectorizedGraphic, vectorize_rgba_to_svg};
use docula_core::{BoundingBox, CropFormat, ImageCrop, Point, Polygon};
use image::{Rgba, RgbaImage};

use crate::buffer::encode_rgba_image;
use crate::error::PreprocessError;

/// Extracted visual region of a document page.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualRegion {
    pub bbox: BoundingBox,
    pub image: RgbaImage,
}

/// Vectorized representation of a visual region containing SVG markup and polygons.
#[derive(Clone, Debug, PartialEq)]
pub struct VectorizedRegion {
    pub bbox: BoundingBox,
    pub width: u32,
    pub height: u32,
    pub polygons: Vec<Polygon>,
    pub svg: String,
    pub path_data: String,
}

impl VectorizedRegion {
    #[inline]
    #[must_use]
    pub fn to_svg(&self) -> &str {
        &self.svg
    }

    #[inline]
    #[must_use]
    pub fn polygon_count(&self) -> usize {
        self.polygons.len()
    }

    #[inline]
    #[must_use]
    pub fn polygons(&self) -> &[Polygon] {
        &self.polygons
    }

    #[inline]
    #[must_use]
    pub fn path_data(&self) -> &str {
        &self.path_data
    }
}

impl VisualRegion {
    #[must_use]
    pub fn new(image: RgbaImage, bbox: BoundingBox) -> Self {
        Self { bbox, image }
    }

    #[must_use]
    pub fn extract_from_image(source: &RgbaImage, bbox: &BoundingBox) -> Self {
        let (w, h) = source.dimensions();
        let rect = bbox.to_pixel_rect(w, h);
        let cropped = image::imageops::crop_imm(source, rect.x, rect.y, rect.width, rect.height);
        Self::new(cropped.to_image(), *bbox)
    }

    #[inline]
    #[must_use]
    pub fn raw_image(&self) -> &RgbaImage {
        &self.image
    }

    #[inline]
    #[must_use]
    pub fn bbox(&self) -> &BoundingBox {
        &self.bbox
    }

    #[inline]
    #[must_use]
    pub fn dimensions(&self) -> (u32, u32) {
        self.image.dimensions()
    }

    #[inline]
    #[must_use]
    pub fn width(&self) -> u32 {
        self.image.width()
    }

    #[inline]
    #[must_use]
    pub fn height(&self) -> u32 {
        self.image.height()
    }

    pub fn to_image_crop(&self, format: CropFormat) -> Result<ImageCrop, PreprocessError> {
        let encoded_bytes = encode_rgba_image(&self.image, format)?;
        let encoded_data = BASE64_STANDARD.encode(&encoded_bytes);
        ImageCrop::new(
            0,
            self.bbox,
            self.width(),
            self.height(),
            encoded_data,
            format.mime_type(),
        )
        .map_err(PreprocessError::Crop)
    }

    #[must_use]
    pub fn mask_only(&self) -> RgbaImage {
        self.mask_with_colors(Rgba([255, 255, 255, 255]), Rgba([0, 0, 0, 255]))
    }

    #[must_use]
    pub fn mask_black_on_white(&self) -> RgbaImage {
        self.mask_with_colors(Rgba([0, 0, 0, 255]), Rgba([255, 255, 255, 255]))
    }

    #[must_use]
    pub fn mask_with_colors(&self, fg_color: Rgba<u8>, bg_color: Rgba<u8>) -> RgbaImage {
        let threshold = self.compute_optimal_threshold();
        let (w, h) = self.dimensions();
        let mut mask = RgbaImage::new(w, h);

        for y in 0..h {
            for x in 0..w {
                let p = self.image.get_pixel(x, y);
                let luma = rgb_to_luma_u8(p[0], p[1], p[2]);
                if luma <= threshold && p[3] > 64 {
                    mask.put_pixel(x, y, fg_color);
                } else {
                    mask.put_pixel(x, y, bg_color);
                }
            }
        }
        mask
    }

    #[must_use]
    pub fn isolate_foreground(&self) -> RgbaImage {
        let threshold = self.compute_optimal_threshold();
        let (w, h) = self.dimensions();
        let mut result = RgbaImage::new(w, h);

        for y in 0..h {
            for x in 0..w {
                let p = self.image.get_pixel(x, y);
                let luma = rgb_to_luma_u8(p[0], p[1], p[2]);
                if luma <= threshold && p[3] > 64 {
                    let alpha = if threshold > 0 {
                        let ratio = (threshold - luma) as f32 / threshold as f32;
                        ((ratio * 180.0 + 75.0).clamp(0.0, 255.0) as u8).min(p[3])
                    } else {
                        p[3]
                    };
                    result.put_pixel(x, y, Rgba([0, 0, 0, alpha]));
                } else {
                    result.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                }
            }
        }
        result
    }

    #[must_use]
    pub fn grayscale(&self) -> RgbaImage {
        let (w, h) = self.dimensions();
        let mut gray = RgbaImage::new(w, h);

        for y in 0..h {
            for x in 0..w {
                let p = self.image.get_pixel(x, y);
                let luma = rgb_to_luma_u8(p[0], p[1], p[2]);
                gray.put_pixel(x, y, Rgba([luma, luma, luma, p[3]]));
            }
        }
        gray
    }

    #[must_use]
    pub fn vectorize(&self) -> VectorizedRegion {
        let config = VectorizeConfig {
            grid_step: 4.0,
            rdp_epsilon: 1.5,
        };
        let graphic: VectorizedGraphic = vectorize_rgba_to_svg(&self.image, config);
        let (w, h) = self.dimensions();

        let mut polygons = Vec::new();
        let mut path_frags = Vec::new();

        for path in &graphic.paths {
            if path.points.len() < 3 {
                continue;
            }
            path_frags.push(path.to_svg_path_data());

            let vertices: Vec<Point> = path
                .points
                .iter()
                .map(|&(px, py)| {
                    Point::new_unchecked(
                        (px / w as f32).clamp(0.0, 1.0),
                        (py / h as f32).clamp(0.0, 1.0),
                    )
                })
                .collect();

            if let Ok(poly) = Polygon::new(vertices) {
                polygons.push(poly);
            }
        }

        let path_data = path_frags.join(" ");
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}"><path d="{path_data}" fill="#000000" fill-rule="evenodd"/></svg>"##
        );

        VectorizedRegion {
            bbox: self.bbox,
            width: w,
            height: h,
            polygons,
            svg,
            path_data,
        }
    }

    fn compute_optimal_threshold(&self) -> u8 {
        let grays: Vec<u8> = self
            .image
            .pixels()
            .map(|p| rgb_to_luma_u8(p[0], p[1], p[2]))
            .collect();
        cv_vectorize::otsu_threshold(&grays)
    }
}
