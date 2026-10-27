use cv_mosaic::unwarp_quad_rgba;
use docula_core::{BoundingBox, DocumentSource};
use docula_preprocess::filter::{OxiMediaClaheFilter, OxiMediaLensUndistortFilter};
use docula_preprocess::{ImageFilter, PageRasterBuffer, PreprocessError};
use docula_vision_codes::OpticalCodeDetector;
use image::{ImageFormat, RgbaImage};
use oximedia_cv::contour::{ContourApprox, ContourConfig, ContourFinder};
use oximedia_cv::fingerprint::phash::{compute_dhash, compute_phash};
use oximedia_cv::instance_segmentation::{InstanceSegmenter, SegmentationConfig as OxiSegConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::ComposerError;

/// Deterministic identity fingerprint for a segmented sub-document instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubDocumentIdentity {
    pub instance_id: String,
    pub reading_order_index: usize,
    pub perceptual_hash: u64,
    pub difference_hash: u64,
    pub optical_identifier: Option<String>,
    pub content_digest: String,
}

/// A rectified and dewarped document instance extracted from a multi-document capture.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentedSubDocument {
    pub identity: SubDocumentIdentity,
    pub buffer: PageRasterBuffer,
    pub original_bounding_box: BoundingBox,
    pub quadrilateral_corners: [(f64, f64); 4],
    pub width: u32,
    pub height: u32,
    pub detection_confidence: f64,
}

impl SegmentedSubDocument {
    #[must_use]
    #[inline]
    pub fn image(&self) -> &RgbaImage {
        &self.buffer.image
    }

    pub fn to_source(&self, format: ImageFormat) -> Result<DocumentSource, ComposerError> {
        self.buffer
            .to_document_source(format)
            .map_err(|e| ComposerError::ImageError(e.to_string()))
    }
}

/// Operational parameters for single-shot multi-document instance segmentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentationConfig {
    pub min_area_ratio: f64,
    pub max_area_ratio: f64,
    pub target_dpi: u32,
    pub scan_optical_codes: bool,
    pub preprocess_subdocs: bool,
    pub aspect_ratio_range: (f64, f64),
    pub luminance_threshold: u8,
}

impl Default for SegmentationConfig {
    fn default() -> Self {
        Self {
            min_area_ratio: 0.01,
            max_area_ratio: 0.95,
            target_dpi: 300,
            scan_optical_codes: true,
            preprocess_subdocs: true,
            aspect_ratio_range: (0.15, 6.0),
            luminance_threshold: 60,
        }
    }
}

/// Engine for detecting, isolating, perspective-rectifying, and fingerprinting multiple sub-documents.
#[derive(Clone, Debug)]
pub struct MultiDocumentSegmenter {
    config: SegmentationConfig,
    code_detector: OpticalCodeDetector,
}

impl Default for MultiDocumentSegmenter {
    fn default() -> Self {
        Self::new(SegmentationConfig::default())
    }
}

impl MultiDocumentSegmenter {
    #[must_use]
    pub fn new(config: SegmentationConfig) -> Self {
        Self {
            config,
            code_detector: OpticalCodeDetector::new(),
        }
    }

    pub fn segment_from_buffer(
        &self,
        parent: &PageRasterBuffer,
    ) -> Result<Vec<SegmentedSubDocument>, ComposerError> {
        self.segment_from_rgba(&parent.image, self.config.target_dpi)
    }

    pub fn segment_from_source(
        &self,
        source: &DocumentSource,
    ) -> Result<Vec<SegmentedSubDocument>, ComposerError> {
        let buf = PageRasterBuffer::from_document_source(source, self.config.target_dpi).map_err(
            |e: PreprocessError| {
                ComposerError::Fragment(format!("Failed to load source for segmentation: {e}"))
            },
        )?;
        self.segment_from_rgba(&buf.image, self.config.target_dpi)
    }

    pub fn segment_from_rgba(
        &self,
        parent: &RgbaImage,
        dpi: u32,
    ) -> Result<Vec<SegmentedSubDocument>, ComposerError> {
        let (canvas_w, canvas_h) = parent.dimensions();
        if canvas_w == 0 || canvas_h == 0 {
            return Err(ComposerError::Fragment("Empty image canvas".to_string()));
        }

        let canvas_area = f64::from(canvas_w) * f64::from(canvas_h);
        let min_area_px = (canvas_area * self.config.min_area_ratio) as u32;

        let oxi_cfg = OxiSegConfig {
            min_area_pixels: min_area_px.max(30),
            max_instances: 32,
            threshold: self.config.luminance_threshold,
        };
        let segmenter = InstanceSegmenter::new(oxi_cfg);
        let masks = segmenter.segment(parent.as_raw(), canvas_w, canvas_h);

        let candidate_bboxes: Vec<((u32, u32, u32, u32), f64)> = if !masks.is_empty() {
            masks
                .into_iter()
                .map(|m| (m.bounding_box, f64::from(m.confidence)))
                .collect()
        } else {
            self.detect_candidate_bboxes_via_contours(parent, canvas_w, canvas_h)
        };

        let mut raw_subdocs = Vec::new();
        for ((x_min, y_min, x_max, y_max), confidence) in candidate_bboxes {
            let width = (x_max.saturating_sub(x_min) + 1).max(1);
            let height = (y_max.saturating_sub(y_min) + 1).max(1);
            let area = f64::from(width) * f64::from(height);
            let area_ratio = area / canvas_area;

            if area_ratio < self.config.min_area_ratio || area_ratio > self.config.max_area_ratio {
                continue;
            }

            let aspect_ratio = f64::from(width) / f64::from(height);
            if aspect_ratio < self.config.aspect_ratio_range.0
                || aspect_ratio > self.config.aspect_ratio_range.1
            {
                continue;
            }

            let quad: [(f64, f64); 4] = [
                (f64::from(x_min), f64::from(y_min)),
                (f64::from(x_max), f64::from(y_min)),
                (f64::from(x_max), f64::from(y_max)),
                (f64::from(x_min), f64::from(y_max)),
            ];

            let rectified_img = unwarp_quad_rgba(parent, &quad, width, height)
                .map_err(|e| ComposerError::ImageError(e.to_string()))?;

            let mut enhanced_img = rectified_img;
            if self.config.preprocess_subdocs {
                let clahe = OxiMediaClaheFilter::default();
                let lens = OxiMediaLensUndistortFilter::default();
                let _ = lens.apply(&mut enhanced_img);
                let _ = clahe.apply(&mut enhanced_img);
            }

            let norm_x0 = (x_min as f32 / canvas_w as f32).clamp(0.0, 1.0);
            let norm_y0 = (y_min as f32 / canvas_h as f32).clamp(0.0, 1.0);
            let norm_x1 = (x_max as f32 / canvas_w as f32).clamp(norm_x0, 1.0);
            let norm_y1 = (y_max as f32 / canvas_h as f32).clamp(norm_y0, 1.0);
            let bbox = BoundingBox::from_coords(norm_x0, norm_y0, norm_x1, norm_y1)
                .map_err(|e| ComposerError::ImageError(e.to_string()))?;

            raw_subdocs.push((bbox, quad, enhanced_img, confidence));
        }

        if raw_subdocs.is_empty() {
            let bbox = BoundingBox::from_coords(0.0, 0.0, 1.0, 1.0)
                .map_err(|e| ComposerError::ImageError(e.to_string()))?;
            let quad = [
                (0.0, 0.0),
                (f64::from(canvas_w), 0.0),
                (f64::from(canvas_w), f64::from(canvas_h)),
                (0.0, f64::from(canvas_h)),
            ];
            raw_subdocs.push((bbox, quad, parent.clone(), 1.0));
        }

        raw_subdocs.sort_by(|a, b| {
            let row_a = (a.0.top_left.y * 10.0).floor() as i32;
            let row_b = (b.0.top_left.y * 10.0).floor() as i32;
            if row_a != row_b {
                row_a.cmp(&row_b)
            } else {
                a.0.top_left
                    .x
                    .partial_cmp(&b.0.top_left.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        let mut results = Vec::with_capacity(raw_subdocs.len());
        for (idx, (bbox, quad, img, confidence)) in raw_subdocs.into_iter().enumerate() {
            let identity = self.compute_subdoc_identity(&img, idx);
            let (w, h) = img.dimensions();
            let buffer = PageRasterBuffer::new(idx, img, dpi)
                .map_err(|e| ComposerError::Preprocessing(e.to_string()))?;

            results.push(SegmentedSubDocument {
                identity,
                buffer,
                original_bounding_box: bbox,
                quadrilateral_corners: quad,
                width: w,
                height: h,
                detection_confidence: confidence,
            });
        }

        Ok(results)
    }

    fn compute_subdoc_identity(
        &self,
        img: &RgbaImage,
        reading_order: usize,
    ) -> SubDocumentIdentity {
        let (w, h) = img.dimensions();
        let mut rgb_buf = Vec::with_capacity((w * h * 3) as usize);
        for p in img.pixels() {
            rgb_buf.push(p[0]);
            rgb_buf.push(p[1]);
            rgb_buf.push(p[2]);
        }

        let perceptual_hash = compute_phash(&rgb_buf, w, h, 8).unwrap_or(0);
        let difference_hash = compute_dhash(&rgb_buf, w, h, 8).unwrap_or(0);

        let optical_identifier = if self.config.scan_optical_codes {
            self.code_detector
                .detect(img)
                .ok()
                .and_then(|codes| codes.first().map(|c| c.raw_content.clone()))
        } else {
            None
        };

        let mut hasher = Sha256::new();
        hasher.update(img.as_raw());
        let digest_bytes = hasher.finalize();
        let content_digest = format!("{digest_bytes:x}");

        let instance_id = if let Some(ref code) = optical_identifier {
            let sanitized_code: String = code
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            format!("subdoc-{reading_order}-{sanitized_code}")
        } else {
            format!("subdoc-{reading_order}-{perceptual_hash:016x}")
        };

        SubDocumentIdentity {
            instance_id,
            reading_order_index: reading_order,
            perceptual_hash,
            difference_hash,
            optical_identifier,
            content_digest,
        }
    }

    fn detect_candidate_bboxes_via_contours(
        &self,
        img: &RgbaImage,
        w: u32,
        h: u32,
    ) -> Vec<((u32, u32, u32, u32), f64)> {
        let thresh = self.config.luminance_threshold;
        let mut binary = Vec::with_capacity((w * h) as usize);

        for p in img.pixels() {
            let luma = cv_mosaic::rgb_to_luma_u8(p[0], p[1], p[2]);
            binary.push(if luma > thresh { 255 } else { 0 });
        }

        let contour_cfg = ContourConfig {
            approx: ContourApprox::Simple,
            min_area: 50.0,
            ..Default::default()
        };
        let finder = ContourFinder::with_config(contour_cfg);
        let contours = finder.find_contours(&binary, w as usize, h as usize);

        let mut bboxes = Vec::new();
        for c in contours {
            if c.is_hole {
                continue;
            }
            if let Some((min_x, min_y, max_x, max_y)) = c.bounding_box() {
                bboxes.push((
                    (
                        min_x.max(0) as u32,
                        min_y.max(0) as u32,
                        max_x.max(0) as u32,
                        max_y.max(0) as u32,
                    ),
                    0.80,
                ));
            }
        }

        bboxes
    }
}
