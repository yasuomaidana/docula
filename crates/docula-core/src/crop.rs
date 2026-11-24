use crate::error::{CropError, DoculaError};
use crate::geometry::BoundingBox;
use serde::{Deserialize, Serialize};

/// Supported image encoding formats for visual screenshot and ROI crop exports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CropFormat {
    /// Portable Network Graphics (lossless).
    #[default]
    Png,
    /// Joint Photographic Experts Group with compression quality (1-100).
    Jpeg {
        /// Compression quality (1-100).
        quality: u8,
    },
    /// WebP image format.
    WebP,
    /// Uncompressed raw RGBA pixel byte buffer.
    RawRgba,
}

impl CropFormat {
    /// Returns standard MIME type string for the format.
    #[inline]
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg { .. } => "image/jpeg",
            Self::WebP => "image/webp",
            Self::RawRgba => "image/x-raw-rgba",
        }
    }

    /// Returns standard file extension for the format without leading dot.
    #[inline]
    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg { .. } => "jpg",
            Self::WebP => "webp",
            Self::RawRgba => "rgba",
        }
    }

    /// Default JPEG configuration with quality 85.
    #[inline]
    pub const fn default_jpeg() -> Self {
        Self::Jpeg { quality: 85 }
    }
}

/// Value object representing a cropped visual region-of-interest (ROI) or full page snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageCrop {
    /// Source document page index (0-indexed).
    pub page_index: usize,
    /// Bounding box in source document coordinate space $[0.0, 1.0]$.
    pub bbox: BoundingBox,
    /// Pixel width of the cropped sub-image.
    pub width: u32,
    /// Pixel height of the cropped sub-image.
    pub height: u32,
    /// Encoded image payload (Base64 data URI or raw hex/data string).
    pub encoded_data: String,
    /// MIME type of the payload (e.g. "image/png").
    pub mime_type: String,
}

impl ImageCrop {
    /// Creates and validates a new `ImageCrop` value object.
    pub fn new(
        page_index: usize,
        bbox: BoundingBox,
        width: u32,
        height: u32,
        encoded_data: impl Into<String>,
        mime_type: impl Into<String>,
    ) -> Result<Self, CropError> {
        if width == 0 || height == 0 {
            return Err(CropError::ZeroDimensions { width, height });
        }

        Ok(Self {
            page_index,
            bbox,
            width,
            height,
            encoded_data: encoded_data.into(),
            mime_type: mime_type.into(),
        })
    }

    /// Aspect ratio of the visual crop ($\text{width} / \text{height}$).
    #[inline]
    pub fn aspect_ratio(&self) -> f32 {
        if self.height > 0 {
            self.width as f32 / self.height as f32
        } else {
            0.0
        }
    }

    /// Returns the length of the encoded payload in characters/bytes.
    #[inline]
    pub fn payload_len(&self) -> usize {
        self.encoded_data.len()
    }
}

/// Core abstraction trait for capturing page screenshots and slicing spatial regions.
///
/// Implemented by preprocessing and rasterization engines (`docula-preprocess`) to decouple
/// heavy rendering dependencies from core domain logic.
pub trait RegionCropper: Send + Sync {
    /// Renders or extracts a full page screenshot at specified DPI and format.
    fn capture_page(
        &self,
        page_index: usize,
        dpi: u32,
        format: CropFormat,
    ) -> Result<ImageCrop, DoculaError>;

    /// Crops a sub-image region from a page defined by a normalized bounding box and padding fraction.
    fn crop_region(
        &self,
        page_index: usize,
        bbox: &BoundingBox,
        padding: f32,
        format: CropFormat,
    ) -> Result<ImageCrop, DoculaError>;
}
