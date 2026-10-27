use cv_mosaic::{
    OverlapAxis, detect_strip_overlap_zmad, estimate_orthogonal_drift_ecc, stitch_frames_feathered,
};
use docula_core::DocumentSource;
use docula_preprocess::PageRasterBuffer;
use image::{ImageFormat, RgbaImage};
use oximedia_cv::registration::panorama as reg_pano;
use serde::{Deserialize, Serialize};

use crate::error::ComposerError;

/// Direction / topology for multi-fragment reconstruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StitchMode {
    #[default]
    VerticalStrip,
    HorizontalStrip,
    PlanarHomography,
    Auto,
}

/// Pairwise alignment relationship between two document fragments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairwiseAlignment {
    pub reference_index: usize,
    pub target_index: usize,
    pub offset_x: i64,
    pub offset_y: i64,
    pub overlap_px: u32,
    pub confidence: f64,
}

/// Operational parameters for multi-shot document reconstruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconstructionConfig {
    pub stitch_mode: StitchMode,
    pub min_overlap_ratio: f64,
    pub max_overlap_ratio: f64,
    pub blend_width_px: u32,
    pub feather_overlap: bool,
    pub apply_bad_shape_filters: bool,
    pub target_dpi: u32,
    pub preprocess_fragments: bool,
}

impl Default for ReconstructionConfig {
    fn default() -> Self {
        Self {
            stitch_mode: StitchMode::Auto,
            min_overlap_ratio: 0.05,
            max_overlap_ratio: 0.60,
            blend_width_px: 24,
            feather_overlap: true,
            apply_bad_shape_filters: false,
            target_dpi: 300,
            preprocess_fragments: false,
        }
    }
}

/// Registration strategy for multi-fragment alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RegistrationMethod {
    #[default]
    PhaseCorrelation,
    FeatureMatching,
    EccOptimization,
    DirectZmad,
}

/// Output of a multi-shot reconstruction operation.
#[derive(Debug, Clone, PartialEq)]
pub struct ReconstructedDocument {
    pub buffer: PageRasterBuffer,
    pub stitched_width: u32,
    pub stitched_height: u32,
    pub alignments: Vec<PairwiseAlignment>,
    pub fragment_count: usize,
    pub estimated_coverage_ratio: f64,
}

impl ReconstructedDocument {
    /// Consumes or accesses the underlying canvas raster image.
    #[must_use]
    pub fn image(&self) -> &RgbaImage {
        &self.buffer.image
    }

    pub fn to_source(&self, format: ImageFormat) -> Result<DocumentSource, ComposerError> {
        self.buffer
            .to_document_source(format)
            .map_err(|e| ComposerError::ImageError(e.to_string()))
    }
}

type AlignmentResult = (Vec<(i64, i64)>, Vec<PairwiseAlignment>);

/// Engine for reconstructing large physical documents from multiple overlapping camera shots.
#[derive(Clone, Debug)]
pub struct DocumentReconstructor {
    config: ReconstructionConfig,
}

impl DocumentReconstructor {
    #[must_use]
    pub fn new(config: ReconstructionConfig) -> Self {
        Self { config }
    }

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

    pub fn reconstruct_from_sources(
        &self,
        sources: &[DocumentSource],
    ) -> Result<ReconstructedDocument, ComposerError> {
        if sources.is_empty() {
            return Err(ComposerError::EmptyFragments);
        }

        let mut images = Vec::with_capacity(sources.len());
        for (idx, source) in sources.iter().enumerate() {
            let buf = PageRasterBuffer::from_document_source(source, self.config.target_dpi)
                .map_err(|e| {
                    ComposerError::Fragment(format!(
                        "Failed to load document fragment {idx} from source: {e}"
                    ))
                })?;
            images.push(buf.image);
        }

        self.reconstruct_from_rgba(&images, self.config.target_dpi)
    }

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

        let (offsets, alignments) = match self.config.stitch_mode {
            StitchMode::VerticalStrip => {
                self.align_strip_sequence(fragments, OverlapAxis::Vertical)?
            }
            StitchMode::HorizontalStrip => {
                self.align_strip_sequence(fragments, OverlapAxis::Horizontal)?
            }
            StitchMode::PlanarHomography => self.align_planar_homography(fragments)?,
            StitchMode::Auto => {
                let first = &fragments[0];
                let second = &fragments[1];
                let (v_overlap, v_err) = detect_strip_overlap_zmad(
                    first,
                    second,
                    OverlapAxis::Vertical,
                    self.config.min_overlap_ratio,
                    self.config.max_overlap_ratio,
                );
                let (h_overlap, h_err) = detect_strip_overlap_zmad(
                    first,
                    second,
                    OverlapAxis::Horizontal,
                    self.config.min_overlap_ratio,
                    self.config.max_overlap_ratio,
                );

                if v_err < h_err && v_overlap > 0 {
                    self.align_strip_sequence(fragments, OverlapAxis::Vertical)?
                } else if h_overlap > 0 {
                    self.align_strip_sequence(fragments, OverlapAxis::Horizontal)?
                } else {
                    self.align_strip_sequence(fragments, OverlapAxis::Vertical)?
                }
            }
        };

        let min_x = offsets.iter().map(|(x, _)| *x).min().unwrap_or(0);
        let min_y = offsets.iter().map(|(_, y)| *y).min().unwrap_or(0);
        let normalized_offsets: Vec<(i64, i64)> = offsets
            .iter()
            .map(|(x, y)| (x - min_x, y - min_y))
            .collect();

        let mut max_x = 0i64;
        let mut max_y = 0i64;
        for (i, (ox, oy)) in normalized_offsets.iter().enumerate() {
            let (w, h) = fragments[i].dimensions();
            max_x = max_x.max(ox + i64::from(w));
            max_y = max_y.max(oy + i64::from(h));
        }

        let stitched_width = max_x.max(1) as u32;
        let stitched_height = max_y.max(1) as u32;

        let frames: Vec<(&RgbaImage, i64, i64)> = fragments
            .iter()
            .zip(normalized_offsets.iter())
            .map(|(img, (ox, oy))| (img, *ox, *oy))
            .collect();

        let blended_canvas = stitch_frames_feathered(&frames, self.config.blend_width_px)
            .map_err(|e| ComposerError::AlignmentFailed(e.to_string()))?;

        let buffer = PageRasterBuffer::new(0, blended_canvas, dpi)
            .map_err(|e| ComposerError::Preprocessing(e.to_string()))?;

        let sum_fragment_areas: f64 = fragments
            .iter()
            .map(|img| f64::from(img.width()) * f64::from(img.height()))
            .sum();
        let stitched_area = f64::from(stitched_width) * f64::from(stitched_height);
        let estimated_coverage_ratio = (stitched_area / sum_fragment_areas.max(1.0)).min(1.0);

        Ok(ReconstructedDocument {
            buffer,
            stitched_width,
            stitched_height,
            alignments,
            fragment_count: fragments.len(),
            estimated_coverage_ratio,
        })
    }

    fn align_strip_sequence(
        &self,
        fragments: &[RgbaImage],
        axis: OverlapAxis,
    ) -> Result<AlignmentResult, ComposerError> {
        let mut offsets = Vec::with_capacity(fragments.len());
        let mut alignments = Vec::with_capacity(fragments.len().saturating_sub(1));

        offsets.push((0i64, 0i64));
        let mut current_x = 0i64;
        let mut current_y = 0i64;

        for i in 0..fragments.len().saturating_sub(1) {
            let ref_img = &fragments[i];
            let tgt_img = &fragments[i + 1];

            let (overlap_px, _err) = detect_strip_overlap_zmad(
                ref_img,
                tgt_img,
                axis,
                self.config.min_overlap_ratio,
                self.config.max_overlap_ratio,
            );
            let drift = estimate_orthogonal_drift_ecc(ref_img, tgt_img, axis, overlap_px);
            let confidence = if overlap_px > 0 { 0.90 } else { 0.50 };

            let (ox, oy) = match axis {
                OverlapAxis::Vertical => (drift, overlap_px as i64),
                OverlapAxis::Horizontal => (overlap_px as i64, drift),
            };

            alignments.push(PairwiseAlignment {
                reference_index: i,
                target_index: i + 1,
                offset_x: ox,
                offset_y: oy,
                overlap_px,
                confidence,
            });

            match axis {
                OverlapAxis::Vertical => {
                    current_x += drift;
                    current_y += i64::from(ref_img.height()) - i64::from(overlap_px);
                }
                OverlapAxis::Horizontal => {
                    current_x += i64::from(ref_img.width()) - i64::from(overlap_px);
                    current_y += drift;
                }
            }
            offsets.push((current_x, current_y));
        }

        Ok((offsets, alignments))
    }

    fn align_planar_homography(
        &self,
        fragments: &[RgbaImage],
    ) -> Result<AlignmentResult, ComposerError> {
        let grays: Vec<Vec<u8>> = fragments
            .iter()
            .map(cv_mosaic::rgba_to_luma_vec)
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
            _ => self.align_strip_sequence(fragments, OverlapAxis::Vertical),
        }
    }
}

/// Detects horizontal overlap between left and right document image sections using Zero-Mean Absolute Difference (ZMAD).
#[must_use]
pub fn detect_overlap_horizontal(left: &RgbaImage, right: &RgbaImage) -> (u32, f64) {
    detect_strip_overlap_zmad(left, right, OverlapAxis::Horizontal, 0.05, 0.75)
}
