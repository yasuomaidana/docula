use super::ImageFilter;
use super::deskew::HoughDeskewFilter;
use super::preprocessed::PreprocessedImage;
use crate::error::PreprocessError;
use image::RgbaImage;

/// Chain of Responsibility for orchestrating sequential image processing filters.
///
/// Filters are executed strictly in the order they were registered. If any filter
/// fails, processing halts immediately and the typed [`PreprocessError`] is returned.
pub struct PreprocessingFilterChain {
    filters: Vec<Box<dyn ImageFilter>>,
}

impl Default for PreprocessingFilterChain {
    fn default() -> Self {
        Self::new()
    }
}

impl PreprocessingFilterChain {
    /// Creates a new empty filter chain.
    #[must_use]
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
        }
    }

    /// Creates a pre-configured standard document OCR pipeline.
    ///
    /// Pipeline order:
    /// 1. [`HoughDeskewFilter`]: Affine rotational correction for tilted baselines.
    #[must_use]
    pub fn default_ocr_pipeline() -> Self {
        Self::new().with_filter(HoughDeskewFilter::new())
    }

    /// Fluent builder method to append a filter to the chain.
    #[must_use]
    pub fn with_filter<F: ImageFilter + 'static>(mut self, filter: F) -> Self {
        self.filters.push(Box::new(filter));
        self
    }

    /// Appends a filter to the chain in place.
    pub fn add_filter<F: ImageFilter + 'static>(&mut self, filter: F) -> &mut Self {
        self.filters.push(Box::new(filter));
        self
    }

    /// Returns the number of filters currently in the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.filters.len()
    }

    /// Returns `true` if the filter chain contains no filters.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }

    /// Returns an immutable slice of all registered filters.
    #[must_use]
    pub fn filters(&self) -> &[Box<dyn ImageFilter>] {
        &self.filters
    }

    /// Executes all filters sequentially on the given RGBA image in place.
    ///
    /// # Errors
    /// Returns [`PreprocessError::InvalidDimensions`] if the input image is empty (0x0).
    /// Returns the respective [`PreprocessError`] if any filter in the chain fails.
    pub fn process(&self, img: &mut RgbaImage) -> Result<(), PreprocessError> {
        let (width, height) = img.dimensions();
        if width == 0 || height == 0 {
            return Err(PreprocessError::InvalidDimensions {
                width,
                height,
                reason: "cannot process zero-dimension image".to_string(),
            });
        }

        for filter in &self.filters {
            filter.apply(img)?;
        }

        Ok(())
    }

    /// Executes the filter chain on the given image and packages the result into a [`PreprocessedImage`].
    ///
    /// The original dimensions are captured before filter passes (e.g. resizes) to maintain
    /// coordinate mapping integrity.
    ///
    /// # Errors
    /// Returns [`PreprocessError`] if any filter fails or image validation fails.
    pub fn process_to_preprocessed(
        &self,
        mut img: RgbaImage,
    ) -> Result<PreprocessedImage, PreprocessError> {
        let (orig_width, orig_height) = img.dimensions();
        self.process(&mut img)?;
        Ok(PreprocessedImage::from_rgba(orig_width, orig_height, img))
    }
}
