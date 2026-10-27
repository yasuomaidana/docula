use docula_core::{BoundingBox, DocumentSource};
use docula_preprocess::filter::{OxiMediaClaheFilter, OxiMediaLensUndistortFilter};
use docula_preprocess::{
    ImageFilter, PageRasterBuffer, PreprocessError, Rasterizer, TinySkiaRasterizer,
};
use docula_vision_codes::OpticalCodeDetector;
use image::{DynamicImage, ImageEncoder, ImageFormat, RgbaImage};
use oximedia_cv::contour::{ContourApprox, ContourConfig, ContourFinder};
use oximedia_cv::fingerprint::phash::{compute_dhash, compute_phash};
use oximedia_cv::instance_segmentation::{InstanceSegmenter, SegmentationConfig as OxiSegConfig};
use oximedia_cv::transform::perspective::{quad_to_rect, warp_perspective};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::ComposerError;

/// Deterministic identity fingerprint for a segmented sub-document instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubDocumentIdentity {
    /// Composite deterministic instance identifier (e.g. "subdoc-0-4a8f9c1e...").
    pub instance_id: String,
    /// Spatial reading order position (0-indexed, top-to-bottom, left-to-right).
    pub reading_order_index: usize,
    /// 64-bit Discrete Cosine Transform perceptual hash (pHash).
    pub perceptual_hash: u64,
    /// 64-bit gradient difference hash (dHash).
    pub difference_hash: u64,
    /// Decoded optical code payload (QR code, Code128, etc.) if detected on the sub-document.
    pub optical_identifier: Option<String>,
    /// Cryptographic SHA-256 hash of the rectified pixel bytes.
    pub content_digest: String,
}

/// A rectified and dewarped document instance extracted from a multi-document capture.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentedSubDocument {
    /// Deterministic identity and tracking metadata.
    pub identity: SubDocumentIdentity,
    /// Rectified page raster buffer ready for downstream OCR.
    pub buffer: PageRasterBuffer,
    /// Axis-aligned bounding box on the parent input image.
    pub original_bounding_box: BoundingBox,
    /// Perspective quadrilateral corner coordinates on the parent image: [(tl), (tr), (br), (bl)].
    pub quadrilateral_corners: [(f64, f64); 4],
    /// Rectified pixel width.
    pub width: u32,
    /// Rectified pixel height.
    pub height: u32,
    /// Detection confidence score $[0.0, 1.0]$.
    pub detection_confidence: f64,
}

impl SegmentedSubDocument {
    /// Reference to the rectified sub-document RGBA image.
    #[must_use]
    #[inline]
    pub fn image(&self) -> &RgbaImage {
        &self.buffer.image
    }

    /// Converts this sub-document into a [`DocumentSource::ImageBytes`] payload.
    ///
    /// # Errors
    /// Returns [`ComposerError::ImageError`] if encoding fails.
    pub fn to_source(&self, format: ImageFormat) -> Result<DocumentSource, ComposerError> {
        let mut encoded = Vec::new();
        match format {
            ImageFormat::Png => {
                let encoder = image::codecs::png::PngEncoder::new(&mut encoded);
                encoder
                    .write_image(
                        self.buffer.image.as_raw(),
                        self.width,
                        self.height,
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|e| ComposerError::ImageError(e.to_string()))?;
            }
            ImageFormat::Jpeg => {
                let mut encoder =
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 90);
                let rgb_img = DynamicImage::ImageRgba8(self.buffer.image.clone()).to_rgb8();
                encoder
                    .encode(
                        rgb_img.as_raw(),
                        self.width,
                        self.height,
                        image::ExtendedColorType::Rgb8,
                    )
                    .map_err(|e| ComposerError::ImageError(e.to_string()))?;
            }
            _ => {
                return Err(ComposerError::ImageError(
                    "Unsupported image encoding format for segmented sub-document".to_string(),
                ));
            }
        }

        let fmt_str = match format {
            ImageFormat::Png => Some("png"),
            ImageFormat::Jpeg => Some("jpeg"),
            _ => None,
        };
        Ok(DocumentSource::from_image_bytes(encoded, fmt_str))
    }
}

/// Operational parameters for single-shot multi-document instance segmentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentationConfig {
    /// Minimum contour area (fraction of image area) to consider a valid sub-document.
    pub min_area_ratio: f64,
    /// Maximum contour area (fraction of image area) to exclude outer canvas borders.
    pub max_area_ratio: f64,
    /// Target rasterization DPI for each extracted sub-document buffer.
    pub target_dpi: u32,
    /// Whether to scan for barcodes or QR codes within each sub-document instance.
    pub scan_optical_codes: bool,
    /// Whether to apply bad-shape physical restoration (CLAHE, lens undistort) on each instance.
    pub preprocess_subdocs: bool,
    /// Acceptable aspect ratio range `(min, max)` for sub-document instances (e.g. 0.2 to 5.0).
    pub aspect_ratio_range: (f64, f64),
    /// Luminance threshold in `[0, 255]` for background separation.
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
    /// Creates a new `MultiDocumentSegmenter` with configuration.
    #[must_use]
    pub fn new(config: SegmentationConfig) -> Self {
        Self {
            config,
            code_detector: OpticalCodeDetector::new(),
        }
    }

    /// Segments multiple documents from a parent [`PageRasterBuffer`].
    ///
    /// # Errors
    /// Returns [`ComposerError`] on failure.
    pub fn segment_from_buffer(
        &self,
        parent: &PageRasterBuffer,
    ) -> Result<Vec<SegmentedSubDocument>, ComposerError> {
        self.segment_from_rgba(&parent.image, self.config.target_dpi)
    }

    /// Segments multiple documents from a parent [`DocumentSource`].
    ///
    /// # Errors
    /// Returns [`ComposerError`] on failure.
    pub fn segment_from_source(
        &self,
        source: &DocumentSource,
    ) -> Result<Vec<SegmentedSubDocument>, ComposerError> {
        let img = load_source_to_rgba(source, self.config.target_dpi).map_err(|e| {
            ComposerError::Fragment(format!("Failed to load source for segmentation: {e}"))
        })?;
        self.segment_from_rgba(&img, self.config.target_dpi)
    }

    /// Segments multiple documents directly from a raw RGBA parent image canvas.
    ///
    /// # Errors
    /// Returns [`ComposerError`] on failure.
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

        // 1. Run Union-Find instance segmentation via OxiMedia
        let oxi_cfg = OxiSegConfig {
            min_area_pixels: min_area_px.max(30),
            max_instances: 32,
            threshold: self.config.luminance_threshold,
        };
        let segmenter = InstanceSegmenter::new(oxi_cfg);
        let masks = segmenter.segment(parent.as_raw(), canvas_w, canvas_h);

        // 2. Fallback to binary contour boundary tracing if Union-Find yields no candidates
        let candidate_bboxes: Vec<((u32, u32, u32, u32), f64)> = if !masks.is_empty() {
            masks
                .into_iter()
                .map(|m| (m.bounding_box, f64::from(m.confidence)))
                .collect()
        } else {
            self.detect_candidate_bboxes_via_contours(parent, canvas_w, canvas_h)
        };

        // 3. Filter candidate instances by area ratio and aspect ratio
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

            // 4. Perspective quad unwarping or crop
            let quad: [(f64, f64); 4] = [
                (f64::from(x_min), f64::from(y_min)),
                (f64::from(x_max), f64::from(y_min)),
                (f64::from(x_max), f64::from(y_max)),
                (f64::from(x_min), f64::from(y_max)),
            ];

            let rectified_img = self.unwarp_sub_image(parent, &quad, width, height)?;

            // 5. Optional physical restoration on sub-document
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

        // If no sub-documents detected, treat parent image as single document instance
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

        // 6. Sort by spatial reading order (top-to-bottom, left-to-right)
        // Group items within 10% vertical distance into the same reading row
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

        // 7. Compute deterministic identity descriptors
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

    /// Computes deterministic multi-factor identity for a rectified sub-document.
    fn compute_subdoc_identity(
        &self,
        img: &RgbaImage,
        reading_order: usize,
    ) -> SubDocumentIdentity {
        let (w, h) = img.dimensions();

        // Prepare packed RGB buffer for OxiMedia pHash and dHash
        let mut rgb_buf = Vec::with_capacity((w * h * 3) as usize);
        for p in img.pixels() {
            rgb_buf.push(p[0]);
            rgb_buf.push(p[1]);
            rgb_buf.push(p[2]);
        }

        let perceptual_hash = compute_phash(&rgb_buf, w, h, 8).unwrap_or(0);
        let difference_hash = compute_dhash(&rgb_buf, w, h, 8).unwrap_or(0);

        // Detect optical barcode or QR code on sub-document if enabled
        let optical_identifier = if self.config.scan_optical_codes {
            self.code_detector
                .detect(img)
                .ok()
                .and_then(|codes| codes.first().map(|c| c.raw_content.clone()))
        } else {
            None
        };

        // SHA-256 content digest
        let mut hasher = Sha256::new();
        hasher.update(img.as_raw());
        let digest_bytes = hasher.finalize();
        let content_digest = format!("{digest_bytes:x}");

        // Deterministic composite ID
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

    /// Unwarps a quadrilateral region from the parent image into a rectified rectangular image.
    fn unwarp_sub_image(
        &self,
        parent: &RgbaImage,
        quad: &[(f64, f64); 4],
        dst_w: u32,
        dst_h: u32,
    ) -> Result<RgbaImage, ComposerError> {
        let (src_w, src_h) = parent.dimensions();

        // If quad is axis-aligned with no perspective slant, do fast direct sub-image crop
        let is_axis_aligned = (quad[0].1 - quad[1].1).abs() < 1.0
            && (quad[3].1 - quad[2].1).abs() < 1.0
            && (quad[0].0 - quad[3].0).abs() < 1.0
            && (quad[1].0 - quad[2].0).abs() < 1.0;

        if is_axis_aligned {
            let x0 = (quad[0].0.max(0.0) as u32).min(src_w.saturating_sub(1));
            let y0 = (quad[0].1.max(0.0) as u32).min(src_h.saturating_sub(1));
            let w = dst_w.min(src_w.saturating_sub(x0));
            let h = dst_h.min(src_h.saturating_sub(y0));

            let mut cropped = RgbaImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    cropped.put_pixel(x, y, *parent.get_pixel(x0 + x, y0 + y));
                }
            }
            return Ok(cropped);
        }

        // Perspective unwarping using DLT homography and inverse bilinear warping
        let transform = quad_to_rect(quad, f64::from(dst_w), f64::from(dst_h)).map_err(|e| {
            ComposerError::ImageError(format!("Homography computation failed: {e}"))
        })?;

        let mut rectified = RgbaImage::new(dst_w, dst_h);
        for c in 0..4 {
            let chan: Vec<u8> = parent.pixels().map(|p| p[c]).collect();
            let warped_c = warp_perspective(&chan, src_w, src_h, &transform, dst_w, dst_h)
                .map_err(|e| {
                    ComposerError::ImageError(format!("Perspective warp channel {c} failed: {e}"))
                })?;

            for y in 0..dst_h {
                for x in 0..dst_w {
                    let idx = (y * dst_w + x) as usize;
                    rectified.get_pixel_mut(x, y)[c] = warped_c[idx];
                }
            }
        }

        Ok(rectified)
    }

    /// Fallback candidate region extraction using contour boundary tracing.
    fn detect_candidate_bboxes_via_contours(
        &self,
        img: &RgbaImage,
        w: u32,
        h: u32,
    ) -> Vec<((u32, u32, u32, u32), f64)> {
        let thresh = self.config.luminance_threshold;
        let mut binary = Vec::with_capacity((w * h) as usize);

        for p in img.pixels() {
            let luma =
                (u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000;
            binary.push(if luma as u8 > thresh { 255 } else { 0 });
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

/// Loads a document source into an in-memory RGBA image.
fn load_source_to_rgba(source: &DocumentSource, dpi: u32) -> Result<RgbaImage, PreprocessError> {
    match source {
        DocumentSource::ImagePath(path) => {
            let img = image::open(path).map_err(|e| PreprocessError::FilterExecution {
                filter: "load_source",
                reason: e.to_string(),
            })?;
            Ok(img.to_rgba8())
        }
        DocumentSource::ImageBytes { data, .. } => {
            let img =
                image::load_from_memory(data).map_err(|e| PreprocessError::FilterExecution {
                    filter: "load_source",
                    reason: e.to_string(),
                })?;
            Ok(img.to_rgba8())
        }
        DocumentSource::PdfBytes(data) => {
            let rasterizer = TinySkiaRasterizer::new();
            let page = rasterizer.render_page(data, 0, dpi)?;
            Ok(page.image)
        }
        DocumentSource::PdfPath(path) => {
            let bytes = std::fs::read(path).map_err(|e| PreprocessError::FilterExecution {
                filter: "load_source",
                reason: e.to_string(),
            })?;
            let rasterizer = TinySkiaRasterizer::new();
            let page = rasterizer.render_page(&bytes, 0, dpi)?;
            Ok(page.image)
        }
        _ => Err(PreprocessError::FilterExecution {
            filter: "load_source",
            reason: "Unsupported document source type for segmentation".to_string(),
        }),
    }
}
