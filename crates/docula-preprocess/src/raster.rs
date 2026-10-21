use crate::buffer::PageRasterBuffer;
use crate::error::PreprocessError;
use docula_core::{BoundingBox, CropError, CropFormat, DoculaError, ImageCrop, RegionCropper};
use pdf_oxide::PdfDocument;
use pdf_oxide::rendering::{RenderOptions, render_page};

/// Strategy trait for rasterizing PDF document pages into memory buffers.
pub trait Rasterizer: Send + Sync {
    /// Renders a single page of a PDF document at specified DPI.
    fn render_page(
        &self,
        pdf_bytes: &[u8],
        page_idx: usize,
        dpi: u32,
    ) -> Result<PageRasterBuffer, PreprocessError>;

    /// Inspects and returns the total page count of the PDF document.
    fn page_count(&self, pdf_bytes: &[u8]) -> Result<usize, PreprocessError>;

    /// Renders all pages of a PDF document at specified DPI into memory buffers.
    fn render_all_pages(
        &self,
        pdf_bytes: &[u8],
        dpi: u32,
    ) -> Result<Vec<PageRasterBuffer>, PreprocessError> {
        let count = self.page_count(pdf_bytes)?;
        let mut pages = Vec::with_capacity(count);
        for idx in 0..count {
            pages.push(self.render_page(pdf_bytes, idx, dpi)?);
        }
        Ok(pages)
    }
}

/// Sovereign pure-Rust PDF rasterizer powered by `pdf_oxide` and `tiny-skia`.
///
/// Implements the [`Rasterizer`] strategy with zero external C dynamic library linkage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TinySkiaRasterizer;

impl TinySkiaRasterizer {
    /// Creates a new `TinySkiaRasterizer` instance.
    #[inline]
    pub const fn new() -> Self {
        Self
    }

    /// Creates a [`DocumentVisualExtractor`] bound to the given PDF bytes.
    #[inline]
    pub fn extractor<'a>(&self, pdf_bytes: &'a [u8]) -> DocumentVisualExtractor<'a> {
        DocumentVisualExtractor::new(pdf_bytes)
    }
}

impl Rasterizer for TinySkiaRasterizer {
    fn render_page(
        &self,
        pdf_bytes: &[u8],
        page_idx: usize,
        dpi: u32,
    ) -> Result<PageRasterBuffer, PreprocessError> {
        if pdf_bytes.is_empty() {
            return Err(PreprocessError::EmptyPayload);
        }
        if dpi == 0 {
            return Err(PreprocessError::InvalidDpi { dpi });
        }

        let doc = PdfDocument::from_bytes(pdf_bytes.to_vec()).map_err(|e| {
            PreprocessError::ParseFailure {
                reason: e.to_string(),
            }
        })?;

        let total_pages = doc
            .page_count()
            .map_err(|e| PreprocessError::ParseFailure {
                reason: e.to_string(),
            })?;

        if page_idx >= total_pages {
            return Err(PreprocessError::PageOutOfBounds {
                page_index: page_idx,
                total_pages,
            });
        }

        let options = RenderOptions::with_dpi(dpi).as_raw();
        let rendered = render_page(&doc, page_idx, &options).map_err(|e| {
            PreprocessError::RasterizationFailure {
                reason: e.to_string(),
            }
        })?;

        let image = image::RgbaImage::from_raw(rendered.width, rendered.height, rendered.data)
            .ok_or_else(|| PreprocessError::RasterizationFailure {
                reason: format!(
                    "Render buffer size mismatch for dimensions: {}x{}",
                    rendered.width, rendered.height
                ),
            })?;

        PageRasterBuffer::new(page_idx, image, dpi)
    }

    fn page_count(&self, pdf_bytes: &[u8]) -> Result<usize, PreprocessError> {
        if pdf_bytes.is_empty() {
            return Err(PreprocessError::EmptyPayload);
        }

        let doc = PdfDocument::from_bytes(pdf_bytes.to_vec()).map_err(|e| {
            PreprocessError::ParseFailure {
                reason: e.to_string(),
            }
        })?;

        doc.page_count().map_err(|e| PreprocessError::ParseFailure {
            reason: e.to_string(),
        })
    }
}

/// PDF visual extractor and region cropper with compliance blackout guarantees.
///
/// Implements [`RegionCropper`] to extract page screenshots and sub-image ROIs while
/// enforcing the Compliance Precedence Invariant (censored bounding boxes are blacked out).
#[derive(Clone, Debug)]
pub struct DocumentVisualExtractor<'a> {
    pdf_bytes: &'a [u8],
    rasterizer: TinySkiaRasterizer,
    default_dpi: u32,
    censor_boxes: Vec<BoundingBox>,
}

impl<'a> DocumentVisualExtractor<'a> {
    /// Creates a new visual extractor for the given PDF bytes with default 150 DPI.
    pub fn new(pdf_bytes: &'a [u8]) -> Self {
        Self {
            pdf_bytes,
            rasterizer: TinySkiaRasterizer::new(),
            default_dpi: 150,
            censor_boxes: Vec::new(),
        }
    }

    /// Sets a custom default DPI for page rasterization.
    pub fn with_dpi(mut self, dpi: u32) -> Self {
        self.default_dpi = dpi;
        self
    }

    /// Configures compliance censor boxes to blackout on all extracted crops.
    pub fn with_censor_boxes(mut self, boxes: Vec<BoundingBox>) -> Self {
        self.censor_boxes = boxes;
        self
    }

    /// Appends an individual censor box.
    pub fn add_censor_box(&mut self, bbox: BoundingBox) {
        self.censor_boxes.push(bbox);
    }

    /// Returns a slice of configured censor boxes.
    #[inline]
    pub fn censor_boxes(&self) -> &[BoundingBox] {
        &self.censor_boxes
    }

    /// Renders a page buffer and applies all configured censor blackouts.
    fn render_censored_page(
        &self,
        page_index: usize,
        dpi: u32,
    ) -> Result<PageRasterBuffer, DoculaError> {
        let mut buffer = self
            .rasterizer
            .render_page(self.pdf_bytes, page_index, dpi)
            .map_err(map_preprocess_to_docula)?;

        for censor in &self.censor_boxes {
            buffer.apply_blackout(censor);
        }

        Ok(buffer)
    }
}

impl RegionCropper for DocumentVisualExtractor<'_> {
    fn capture_page(
        &self,
        page_index: usize,
        dpi: u32,
        format: CropFormat,
    ) -> Result<ImageCrop, DoculaError> {
        let buffer = self.render_censored_page(page_index, dpi)?;
        let unit_box = BoundingBox::from_coords_clamped(0.0, 0.0, 1.0, 1.0);
        buffer
            .crop_to_image_crop(&unit_box, format)
            .map_err(map_preprocess_to_docula)
    }

    fn crop_region(
        &self,
        page_index: usize,
        bbox: &BoundingBox,
        padding: f32,
        format: CropFormat,
    ) -> Result<ImageCrop, DoculaError> {
        let buffer = self.render_censored_page(page_index, self.default_dpi)?;

        let target_box = if padding > 0.0 {
            BoundingBox::from_coords_clamped(
                bbox.top_left.x - padding,
                bbox.top_left.y - padding,
                bbox.bottom_right.x + padding,
                bbox.bottom_right.y + padding,
            )
        } else {
            *bbox
        };

        buffer
            .crop_to_image_crop(&target_box, format)
            .map_err(map_preprocess_to_docula)
    }
}

/// Type alias maintaining full backward compatibility with earlier API references.
pub type PdfRegionCropper<'a> = DocumentVisualExtractor<'a>;

/// Helper to map `PreprocessError` to `DoculaError`.
fn map_preprocess_to_docula(err: PreprocessError) -> DoculaError {
    match err {
        PreprocessError::Crop(c) => DoculaError::Crop(c),
        PreprocessError::Geometry(g) => DoculaError::Geometry(g),
        PreprocessError::Core(c) => c,
        PreprocessError::PageOutOfBounds {
            page_index,
            total_pages,
        } => DoculaError::Crop(CropError::PageOutOfBounds {
            page_index,
            total_pages,
        }),
        other => DoculaError::Crop(CropError::EncodingFailed {
            reason: other.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tiny_skia_rasterizer_constructors() {
        let r1 = TinySkiaRasterizer::new();
        let r2 = TinySkiaRasterizer;
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_error_mapping() {
        let err1 = PreprocessError::PageOutOfBounds {
            page_index: 3,
            total_pages: 1,
        };
        let docula_err1 = map_preprocess_to_docula(err1);
        assert!(matches!(
            docula_err1,
            DoculaError::Crop(CropError::PageOutOfBounds {
                page_index: 3,
                total_pages: 1
            })
        ));

        let err2 = PreprocessError::EmptyPayload;
        let docula_err2 = map_preprocess_to_docula(err2);
        assert!(matches!(
            docula_err2,
            DoculaError::Crop(CropError::EncodingFailed { .. })
        ));
    }
}
