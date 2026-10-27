use docula_core::DocumentSource;
use docula_preprocess::ImageFilter;
use docula_preprocess::filter::{
    OxiMediaClaheFilter, OxiMediaLensUndistortFilter, OxiMediaStitcher,
};
use docula_preprocess::{PageRasterBuffer, PreprocessError, Rasterizer, TinySkiaRasterizer};
use image::{DynamicImage, ImageEncoder, ImageFormat, RgbaImage};
use oximedia_cv::registration::TransformationType;
use oximedia_cv::registration::ecc::register_ecc;
use oximedia_cv::registration::panorama as reg_pano;
use serde::{Deserialize, Serialize};

use crate::error::ComposerError;

/// Alignment and composition strategy for combining document fragments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StitchMode {
    /// Multi-frame vertical sequence (top-to-bottom document scanning).
    VerticalStrip,
    /// Multi-frame horizontal sequence (left-to-right table or wide sheet scanning).
    HorizontalStrip,
    /// 2D planar feature matching and homography estimation via RANSAC.
    PlanarHomography,
    /// Automatically detects orientation based on aspect ratio and overlap correlation.
    #[default]
    Auto,
}

/// Algorithmic method used to register overlapping document areas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RegistrationMethod {
    /// FFT-based 2D phase correlation.
    PhaseCorrelation,
    /// Enhanced Correlation Coefficient optimization.
    Ecc,
    /// FAST/ORB feature matching with RANSAC homography.
    FeatureHomography,
    /// Automatic detection based on image structure and dimensions.
    #[default]
    Auto,
}

/// Operational configuration for large document reconstruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconstructionConfig {
    /// Desired stitching orientation.
    pub stitch_mode: StitchMode,
    /// Registration algorithm for computing offsets.
    pub registration_method: RegistrationMethod,
    /// Whether to apply linear alpha feathering across seams.
    pub feather_overlap: bool,
    /// Width of the alpha blending transition in pixels.
    pub blend_width_px: u32,
    /// Minimum fractional overlap between consecutive slices (e.g. 0.05 = 5%).
    pub min_overlap_ratio: f64,
    /// Maximum fractional overlap between consecutive slices (e.g. 0.75 = 75%).
    pub max_overlap_ratio: f64,
    /// Whether to apply bad-shape physical restoration (CLAHE, dewarp, undistort) prior to stitching.
    pub apply_bad_shape_filters: bool,
    /// Target rasterization DPI for reconstructed document.
    pub target_dpi: u32,
}

impl Default for ReconstructionConfig {
    fn default() -> Self {
        Self {
            stitch_mode: StitchMode::Auto,
            registration_method: RegistrationMethod::Auto,
            feather_overlap: true,
            blend_width_px: 32,
            min_overlap_ratio: 0.05,
            max_overlap_ratio: 0.75,
            apply_bad_shape_filters: true,
            target_dpi: 300,
        }
    }
}

/// Result of registering a pair of adjacent document fragments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairwiseAlignment {
    /// Index of reference frame.
    pub reference_index: usize,
    /// Index of target frame.
    pub target_index: usize,
    /// Computed horizontal pixel shift relative to reference.
    pub offset_x: i64,
    /// Computed vertical pixel shift relative to reference.
    pub offset_y: i64,
    /// Overlap dimension in pixels along primary axis.
    pub overlap_px: u32,
    /// Alignment confidence score $[0.0, 1.0]$.
    pub confidence: f64,
}

/// Unified document synthesized from multiple overlapping image captures.
#[derive(Clone, Debug, PartialEq)]
pub struct ReconstructedDocument {
    /// The merged and preprocessed document raster buffer.
    pub buffer: PageRasterBuffer,
    /// Total width of the unified canvas in pixels.
    pub stitched_width: u32,
    /// Total height of the unified canvas in pixels.
    pub stitched_height: u32,
    /// Computed pairwise alignments between input fragments.
    pub alignments: Vec<PairwiseAlignment>,
    /// Total count of source fragments merged into this canvas.
    pub fragment_count: usize,
    /// Estimated non-overlapping coverage ratio over sum of individual areas.
    pub estimated_coverage_ratio: f64,
}

impl ReconstructedDocument {
    /// Reference to the underlying RGBA image.
    #[must_use]
    #[inline]
    pub fn image(&self) -> &RgbaImage {
        &self.buffer.image
    }

    /// Converts the reconstructed canvas into a [`DocumentSource::ImageBytes`] payload.
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
                        self.stitched_width,
                        self.stitched_height,
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|e| ComposerError::ImageError(e.to_string()))?;
            }
            ImageFormat::Jpeg => {
                let mut encoder =
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 90);
                // Convert RGBA to RGB for JPEG encoding
                let rgb_img = DynamicImage::ImageRgba8(self.buffer.image.clone()).to_rgb8();
                encoder
                    .encode(
                        rgb_img.as_raw(),
                        self.stitched_width,
                        self.stitched_height,
                        image::ExtendedColorType::Rgb8,
                    )
                    .map_err(|e| ComposerError::ImageError(e.to_string()))?;
            }
            _ => {
                return Err(ComposerError::ImageError(
                    "Unsupported image encoding format for reconstructed document".to_string(),
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

/// Pairwise offset mapping and alignments across multiple fragments.
type AlignmentResult = (Vec<(i64, i64)>, Vec<PairwiseAlignment>);

/// Engine for reconstructing large physical documents from multiple overlapping camera shots.
#[derive(Clone, Debug)]
pub struct DocumentReconstructor {
    config: ReconstructionConfig,
}

impl DocumentReconstructor {
    /// Creates a new `DocumentReconstructor` with the specified configuration.
    #[must_use]
    pub fn new(config: ReconstructionConfig) -> Self {
        Self { config }
    }

    /// Reconstructs a single document canvas from a collection of rasterized page buffers.
    ///
    /// # Errors
    /// Returns [`ComposerError`] if fragments are empty or stitching fails.
    pub fn reconstruct_from_buffers(
        &self,
        fragments: &[PageRasterBuffer],
    ) -> Result<ReconstructedDocument, ComposerError> {
        if fragments.is_empty() {
            return Err(ComposerError::EmptyFragments);
        }
        if fragments.len() == 1 {
            let single = &fragments[0];
            return Ok(ReconstructedDocument {
                buffer: single.clone(),
                stitched_width: single.width(),
                stitched_height: single.height(),
                alignments: Vec::new(),
                fragment_count: 1,
                estimated_coverage_ratio: 1.0,
            });
        }

        let images: Vec<RgbaImage> = fragments.iter().map(|f| f.image.clone()).collect();
        self.reconstruct_from_rgba(&images, self.config.target_dpi)
    }

    /// Reconstructs a document canvas from a collection of [`DocumentSource`] payloads.
    ///
    /// # Errors
    /// Returns [`ComposerError`] if sources cannot be loaded or stitching fails.
    pub fn reconstruct_from_sources(
        &self,
        sources: &[DocumentSource],
    ) -> Result<ReconstructedDocument, ComposerError> {
        if sources.is_empty() {
            return Err(ComposerError::EmptyFragments);
        }

        let mut images = Vec::with_capacity(sources.len());
        for (idx, source) in sources.iter().enumerate() {
            let img = load_source_to_rgba(source, self.config.target_dpi).map_err(|e| {
                ComposerError::Fragment(format!(
                    "Failed to load document fragment {idx} from source: {e}"
                ))
            })?;
            images.push(img);
        }

        self.reconstruct_from_rgba(&images, self.config.target_dpi)
    }

    /// Reconstructs a unified document canvas directly from RGBA image slices.
    ///
    /// # Errors
    /// Returns [`ComposerError`] if inputs are empty or alignment fails.
    pub fn reconstruct_from_rgba(
        &self,
        fragments: &[RgbaImage],
        dpi: u32,
    ) -> Result<ReconstructedDocument, ComposerError> {
        if fragments.is_empty() {
            return Err(ComposerError::EmptyFragments);
        }
        if fragments.len() == 1 {
            let img = fragments[0].clone();
            let (w, h) = img.dimensions();
            let buffer = PageRasterBuffer::new(0, img, dpi)
                .map_err(|e| ComposerError::Preprocessing(e.to_string()))?;
            return Ok(ReconstructedDocument {
                buffer,
                stitched_width: w,
                stitched_height: h,
                alignments: Vec::new(),
                fragment_count: 1,
                estimated_coverage_ratio: 1.0,
            });
        }

        // 1. Determine stitching orientation
        let mode = match self.config.stitch_mode {
            StitchMode::Auto => self.detect_stitching_mode(fragments),
            explicit => explicit,
        };

        // 2. Compute pairwise alignments and absolute canvas coordinates
        let (offsets, alignments) = match mode {
            StitchMode::VerticalStrip => self.align_vertical_strip(fragments)?,
            StitchMode::HorizontalStrip => self.align_horizontal_strip(fragments)?,
            StitchMode::PlanarHomography | StitchMode::Auto => {
                self.align_planar_homography(fragments)?
            }
        };

        // 3. Normalize coordinates so (min_x, min_y) == (0, 0)
        let min_x = offsets.iter().map(|(ox, _)| *ox).min().unwrap_or(0);
        let min_y = offsets.iter().map(|(_, oy)| *oy).min().unwrap_or(0);
        let normalized_offsets: Vec<(i64, i64)> = offsets
            .iter()
            .map(|(ox, oy)| (ox - min_x, oy - min_y))
            .collect();

        // 4. Compose frames using alpha-feathered multi-frame stitching
        let frame_tuples: Vec<(&RgbaImage, i64, i64)> = fragments
            .iter()
            .zip(normalized_offsets.iter())
            .map(|(img, (ox, oy))| (img, *ox, *oy))
            .collect();

        let blend_width = if self.config.feather_overlap {
            self.config.blend_width_px
        } else {
            0
        };

        let mut merged_canvas =
            OxiMediaStitcher::stitch_frames_feathered(&frame_tuples, blend_width)
                .map_err(|e| ComposerError::AlignmentFailed(e.to_string()))?;

        // 5. Apply physical restoration filters to the unified reconstructed canvas
        if self.config.apply_bad_shape_filters {
            let clahe = OxiMediaClaheFilter::default().with_background_division(Some(16));
            let lens = OxiMediaLensUndistortFilter::default();
            let _ = lens.apply(&mut merged_canvas);
            let _ = clahe.apply(&mut merged_canvas);
        }

        let (stitched_width, stitched_height) = merged_canvas.dimensions();

        // 6. Compute coverage ratio
        let total_fragment_area: u64 = fragments
            .iter()
            .map(|f| u64::from(f.width()) * u64::from(f.height()))
            .sum();
        let unified_area = u64::from(stitched_width) * u64::from(stitched_height);
        let estimated_coverage_ratio = if total_fragment_area > 0 {
            (unified_area as f64 / total_fragment_area as f64).min(1.0)
        } else {
            1.0
        };

        let buffer = PageRasterBuffer::new(0, merged_canvas, dpi)
            .map_err(|e| ComposerError::Preprocessing(e.to_string()))?;

        Ok(ReconstructedDocument {
            buffer,
            stitched_width,
            stitched_height,
            alignments,
            fragment_count: fragments.len(),
            estimated_coverage_ratio,
        })
    }

    /// Automatically infers whether fragments are aligned vertically, horizontally, or planarly.
    fn detect_stitching_mode(&self, fragments: &[RgbaImage]) -> StitchMode {
        if fragments.len() < 2 {
            return StitchMode::VerticalStrip;
        }

        let first = &fragments[0];
        let second = &fragments[1];

        // 1. Measure best vertical and horizontal overlaps and their Zero-Mean errors
        let (vert_overlap, vert_err) = self.measure_vertical_overlap(first, second);
        let (horiz_overlap, horiz_err) = detect_overlap_horizontal(first, second);

        if vert_err <= horiz_err && vert_overlap > 8 {
            StitchMode::VerticalStrip
        } else if horiz_overlap > 8 {
            StitchMode::HorizontalStrip
        } else {
            StitchMode::PlanarHomography
        }
    }

    /// Computes consecutive offsets for a vertical strip scan.
    fn align_vertical_strip(
        &self,
        fragments: &[RgbaImage],
    ) -> Result<AlignmentResult, ComposerError> {
        let mut offsets = Vec::with_capacity(fragments.len());
        let mut alignments = Vec::with_capacity(fragments.len().saturating_sub(1));

        offsets.push((0i64, 0i64));

        let mut current_y = 0i64;
        let mut current_x = 0i64;

        for i in 0..fragments.len().saturating_sub(1) {
            let ref_img = &fragments[i];
            let tgt_img = &fragments[i + 1];

            let (overlap_y, _err) = self.measure_vertical_overlap(ref_img, tgt_img);
            let drift_x = self.measure_horizontal_drift(ref_img, tgt_img, overlap_y);

            let confidence = if overlap_y > 0 { 0.90 } else { 0.50 };

            alignments.push(PairwiseAlignment {
                reference_index: i,
                target_index: i + 1,
                offset_x: drift_x,
                offset_y: overlap_y as i64,
                overlap_px: overlap_y,
                confidence,
            });

            current_x += drift_x;
            current_y += i64::from(ref_img.height()) - i64::from(overlap_y);
            offsets.push((current_x, current_y));
        }

        Ok((offsets, alignments))
    }

    /// Computes consecutive offsets for a horizontal strip scan.
    fn align_horizontal_strip(
        &self,
        fragments: &[RgbaImage],
    ) -> Result<AlignmentResult, ComposerError> {
        let mut offsets = Vec::with_capacity(fragments.len());
        let mut alignments = Vec::with_capacity(fragments.len().saturating_sub(1));

        offsets.push((0i64, 0i64));

        let mut current_x = 0i64;
        let mut current_y = 0i64;

        for i in 0..fragments.len().saturating_sub(1) {
            let ref_img = &fragments[i];
            let tgt_img = &fragments[i + 1];

            let (overlap_x, _err) = detect_overlap_horizontal(ref_img, tgt_img);
            let drift_y = self.measure_vertical_drift(ref_img, tgt_img, overlap_x);

            let confidence = if overlap_x > 0 { 0.90 } else { 0.50 };

            alignments.push(PairwiseAlignment {
                reference_index: i,
                target_index: i + 1,
                offset_x: overlap_x as i64,
                offset_y: drift_y,
                overlap_px: overlap_x,
                confidence,
            });

            current_x += i64::from(ref_img.width()) - i64::from(overlap_x);
            current_y += drift_y;
            offsets.push((current_x, current_y));
        }

        Ok((offsets, alignments))
    }

    /// Computes planar homography chain alignments using `oximedia_cv::registration::panorama`.
    fn align_planar_homography(
        &self,
        fragments: &[RgbaImage],
    ) -> Result<AlignmentResult, ComposerError> {
        let grays: Vec<Vec<u8>> = fragments
            .iter()
            .map(|img| {
                img.pixels()
                    .map(|p| {
                        ((u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114)
                            / 1000) as u8
                    })
                    .collect()
            })
            .collect();

        let image_slices: Vec<(&[u8], u32, u32)> = grays
            .iter()
            .zip(fragments.iter())
            .map(|(gray, img)| (gray.as_slice(), img.width(), img.height()))
            .collect();

        let stitcher = reg_pano::PanoramaStitcher::new();
        match stitcher.compute_chain_homographies(&image_slices) {
            Ok(homographies) if homographies.len() == fragments.len() => {
                let mut offsets = Vec::with_capacity(fragments.len());
                let mut alignments = Vec::with_capacity(fragments.len().saturating_sub(1));

                for (idx, h) in homographies.iter().enumerate() {
                    let ox = h.data[2].round() as i64;
                    let oy = h.data[5].round() as i64;
                    offsets.push((ox, oy));

                    if idx > 0 {
                        alignments.push(PairwiseAlignment {
                            reference_index: idx - 1,
                            target_index: idx,
                            offset_x: ox,
                            offset_y: oy,
                            overlap_px: (ox.unsigned_abs().min(oy.unsigned_abs())) as u32,
                            confidence: 0.85,
                        });
                    }
                }
                Ok((offsets, alignments))
            }
            _ => {
                // Fallback to vertical strip alignment if planar feature extraction cannot chain
                self.align_vertical_strip(fragments)
            }
        }
    }

    /// Measures vertical overlap between two consecutive images using Zero-Mean Absolute Difference (ZMAD).
    fn measure_vertical_overlap(&self, top: &RgbaImage, bottom: &RgbaImage) -> (u32, f64) {
        let (w_top, h_top) = top.dimensions();
        let (w_bot, h_bot) = bottom.dimensions();
        let w = w_top.min(w_bot);

        let min_overlap = ((h_top as f64 * self.config.min_overlap_ratio) as u32).max(12);
        let max_overlap = ((h_top as f64 * self.config.max_overlap_ratio) as u32)
            .min(h_top.saturating_sub(1))
            .min(h_bot.saturating_sub(1));

        if max_overlap <= min_overlap {
            return (min_overlap, f64::MAX);
        }

        let mut best_overlap = min_overlap;
        let mut min_avg_diff = f64::MAX;

        for cand in min_overlap..=max_overlap {
            // 1. Compute mean intensity of top strip and bottom strip
            let mut sum1 = 0u64;
            let mut sum2 = 0u64;
            let mut count = 0u64;

            let step_r = (cand / 15).max(1);
            for r in (0..cand).step_by(step_r as usize) {
                let y_top = h_top - cand + r;
                let y_bot = r;
                for x in (0..w).step_by(3) {
                    let p1 = top.get_pixel(x, y_top);
                    let l1 = (u32::from(p1[0]) + u32::from(p1[1]) + u32::from(p1[2])) / 3;
                    sum1 += u64::from(l1);

                    let p2 = bottom.get_pixel(x, y_bot);
                    let l2 = (u32::from(p2[0]) + u32::from(p2[1]) + u32::from(p2[2])) / 3;
                    sum2 += u64::from(l2);

                    count += 1;
                }
            }

            if count == 0 {
                continue;
            }

            let mean1 = sum1 as f64 / count as f64;
            let mean2 = sum2 as f64 / count as f64;

            // 2. Compute Zero-Mean Absolute Difference (ZMAD)
            let mut diff_sum = 0.0f64;
            for r in (0..cand).step_by(step_r as usize) {
                let y_top = h_top - cand + r;
                let y_bot = r;
                for x in (0..w).step_by(3) {
                    let p1 = top.get_pixel(x, y_top);
                    let l1 = (u32::from(p1[0]) + u32::from(p1[1]) + u32::from(p1[2])) as f64 / 3.0;

                    let p2 = bottom.get_pixel(x, y_bot);
                    let l2 = (u32::from(p2[0]) + u32::from(p2[1]) + u32::from(p2[2])) as f64 / 3.0;

                    let zero_mean_1 = l1 - mean1;
                    let zero_mean_2 = l2 - mean2;
                    diff_sum += (zero_mean_1 - zero_mean_2).abs();
                }
            }

            let avg_diff = diff_sum / count as f64;
            if avg_diff < min_avg_diff {
                min_avg_diff = avg_diff;
                best_overlap = cand;
            }
        }

        (best_overlap, min_avg_diff)
    }

    /// Measures small horizontal drift `dx` between overlapping slices.
    fn measure_horizontal_drift(
        &self,
        top: &RgbaImage,
        bottom: &RgbaImage,
        overlap_height: u32,
    ) -> i64 {
        if overlap_height < 4 {
            return 0;
        }

        let (w1, h1) = top.dimensions();
        let (w2, _h2) = bottom.dimensions();
        let width = w1.min(w2);

        // Extract grayscale strips in the overlap zone
        let mut top_strip = Vec::with_capacity((width * overlap_height) as usize);
        let mut bot_strip = Vec::with_capacity((width * overlap_height) as usize);

        let start_y = h1.saturating_sub(overlap_height);
        for y in 0..overlap_height {
            for x in 0..width {
                let p1 = top.get_pixel(x, start_y + y);
                let l1 = (u32::from(p1[0]) + u32::from(p1[1]) + u32::from(p1[2])) / 3;
                top_strip.push(l1 as u8);

                let p2 = bottom.get_pixel(x, y);
                let l2 = (u32::from(p2[0]) + u32::from(p2[1]) + u32::from(p2[2])) / 3;
                bot_strip.push(l2 as u8);
            }
        }

        // Attempt ECC translation refinement
        if let Ok((matrix, _quality)) = register_ecc(
            &top_strip,
            &bot_strip,
            width,
            overlap_height,
            TransformationType::Translation,
            15,
            1e-3,
        ) {
            let dx = matrix.data[2].round() as i64;
            if dx.abs() < (width as i64 / 4) {
                return dx;
            }
        }

        0
    }

    /// Measures small vertical drift `dy` between horizontally overlapping slices.
    fn measure_vertical_drift(
        &self,
        left: &RgbaImage,
        right: &RgbaImage,
        overlap_width: u32,
    ) -> i64 {
        if overlap_width < 4 {
            return 0;
        }

        let (w1, h1) = left.dimensions();
        let (_w2, h2) = right.dimensions();
        let height = h1.min(h2);

        let mut left_strip = Vec::with_capacity((overlap_width * height) as usize);
        let mut right_strip = Vec::with_capacity((overlap_width * height) as usize);

        let start_x = w1.saturating_sub(overlap_width);
        for y in 0..height {
            for x in 0..overlap_width {
                let p1 = left.get_pixel(start_x + x, y);
                let l1 = (u32::from(p1[0]) + u32::from(p1[1]) + u32::from(p1[2])) / 3;
                left_strip.push(l1 as u8);

                let p2 = right.get_pixel(x, y);
                let l2 = (u32::from(p2[0]) + u32::from(p2[1]) + u32::from(p2[2])) / 3;
                right_strip.push(l2 as u8);
            }
        }

        if let Ok((matrix, _quality)) = register_ecc(
            &left_strip,
            &right_strip,
            overlap_width,
            height,
            TransformationType::Translation,
            15,
            1e-3,
        ) {
            let dy = matrix.data[5].round() as i64;
            if dy.abs() < (height as i64 / 4) {
                return dy;
            }
        }

        0
    }
}

/// Detects horizontal overlap between left and right document image sections using Zero-Mean Absolute Difference (ZMAD).
#[must_use]
pub fn detect_overlap_horizontal(left: &RgbaImage, right: &RgbaImage) -> (u32, f64) {
    let (w_left, h_left) = left.dimensions();
    let (w_right, h_right) = right.dimensions();
    let h = h_left.min(h_right);

    let min_overlap = 12u32;
    let max_overlap = (w_left * 3 / 4)
        .min(w_right * 3 / 4)
        .min(w_left.saturating_sub(1))
        .max(12);

    if max_overlap <= min_overlap {
        return (min_overlap, f64::MAX);
    }

    let mut best_overlap = min_overlap;
    let mut min_avg_diff = f64::MAX;

    for cand in min_overlap..=max_overlap {
        let mut sum1 = 0u64;
        let mut sum2 = 0u64;
        let mut count = 0u64;

        let step_c = (cand / 15).max(1);
        for c in (0..cand).step_by(step_c as usize) {
            let x_left = w_left - cand + c;
            let x_right = c;
            for y in (0..h).step_by(3) {
                let p1 = left.get_pixel(x_left, y);
                let l1 = (u32::from(p1[0]) + u32::from(p1[1]) + u32::from(p1[2])) / 3;
                sum1 += u64::from(l1);

                let p2 = right.get_pixel(x_right, y);
                let l2 = (u32::from(p2[0]) + u32::from(p2[1]) + u32::from(p2[2])) / 3;
                sum2 += u64::from(l2);

                count += 1;
            }
        }

        if count == 0 {
            continue;
        }

        let mean1 = sum1 as f64 / count as f64;
        let mean2 = sum2 as f64 / count as f64;

        let mut diff_sum = 0.0f64;
        for c in (0..cand).step_by(step_c as usize) {
            let x_left = w_left - cand + c;
            let x_right = c;
            for y in (0..h).step_by(3) {
                let p1 = left.get_pixel(x_left, y);
                let l1 = (u32::from(p1[0]) + u32::from(p1[1]) + u32::from(p1[2])) as f64 / 3.0;

                let p2 = right.get_pixel(x_right, y);
                let l2 = (u32::from(p2[0]) + u32::from(p2[1]) + u32::from(p2[2])) as f64 / 3.0;

                let zero_mean_1 = l1 - mean1;
                let zero_mean_2 = l2 - mean2;
                diff_sum += (zero_mean_1 - zero_mean_2).abs();
            }
        }

        let avg_diff = diff_sum / count as f64;
        if avg_diff < min_avg_diff {
            min_avg_diff = avg_diff;
            best_overlap = cand;
        }
    }

    (best_overlap, min_avg_diff)
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
            reason: "Unsupported document source type for reconstruction".to_string(),
        }),
    }
}
