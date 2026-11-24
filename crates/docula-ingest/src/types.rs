use docula_core::SpatialSpan;
use serde::{Deserialize, Serialize};

/// Routing classification for a single document page.
#[derive(Clone, Debug, PartialEq)]
pub enum PageRoute {
    /// Born-digital page with native text layer extracted directly from PDF streams.
    BornDigital {
        /// 0-indexed page number.
        page_index: usize,
        /// Extracted spatial text spans with normalized coordinates $[0.0, 1.0]$.
        text_layer: Vec<SpatialSpan>,
    },
    /// Scanned page, corrupt font map, or image requiring rasterization and deep learning OCR.
    NeedsOcr {
        /// 0-indexed page number.
        page_index: usize,
        /// Parameters for the downstream rasterization and OCR job.
        raster_job: RasterJob,
    },
}

impl PageRoute {
    /// Returns the 0-indexed page number of this route.
    #[must_use]
    pub fn page_index(&self) -> usize {
        match self {
            Self::BornDigital { page_index, .. } | Self::NeedsOcr { page_index, .. } => *page_index,
        }
    }

    /// Returns `true` if the page is classified as born-digital.
    #[must_use]
    pub fn is_born_digital(&self) -> bool {
        matches!(self, Self::BornDigital { .. })
    }

    /// Returns `true` if the page requires neural OCR.
    #[must_use]
    pub fn needs_ocr(&self) -> bool {
        matches!(self, Self::NeedsOcr { .. })
    }
}

/// Specifications for a page rasterization and OCR execution task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RasterJob {
    /// 0-indexed page number.
    pub page_index: usize,
    /// Target rasterization resolution in DPI (default 300).
    pub dpi: u32,
}

impl RasterJob {
    /// Creates a new `RasterJob` with target DPI.
    #[must_use]
    pub const fn new(page_index: usize, dpi: u32) -> Self {
        Self { page_index, dpi }
    }
}

/// Aggregated routing report for all pages of an ingested document.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentRoutingReport {
    /// Total number of pages evaluated.
    pub total_pages: usize,
    /// Ordered routing decisions for all pages.
    pub routes: Vec<PageRoute>,
}

impl DocumentRoutingReport {
    /// Constructs a report from a sequence of page routes.
    #[must_use]
    pub fn new(routes: Vec<PageRoute>) -> Self {
        Self {
            total_pages: routes.len(),
            routes,
        }
    }

    /// Count of pages successfully routed to the born-digital fast path.
    #[must_use]
    pub fn born_digital_count(&self) -> usize {
        self.routes.iter().filter(|r| r.is_born_digital()).count()
    }

    /// Count of pages routed to the OCR pipeline.
    #[must_use]
    pub fn needs_ocr_count(&self) -> usize {
        self.routes.iter().filter(|r| r.needs_ocr()).count()
    }

    /// Returns `true` if all pages in the document are born-digital.
    #[must_use]
    pub fn is_entirely_digital(&self) -> bool {
        self.total_pages > 0 && self.born_digital_count() == self.total_pages
    }

    /// Returns `true` if any page in the document requires OCR.
    #[must_use]
    pub fn requires_ocr(&self) -> bool {
        self.needs_ocr_count() > 0
    }
}

/// Heuristic thresholds for the [`DocumentRouter`](crate::DocumentRouter).
#[derive(Clone, Debug)]
pub struct RouterConfig {
    /// Minimum non-whitespace character count required for a page to qualify as digital.
    pub min_text_chars: usize,
    /// Default DPI for pages routed to OCR.
    pub default_dpi: u32,
    /// Maximum allowed ratio of replacement characters (`\u{FFFD}`) before routing to OCR.
    pub max_replacement_char_ratio: f32,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            min_text_chars: 10,
            default_dpi: 300,
            max_replacement_char_ratio: 0.15,
        }
    }
}

impl RouterConfig {
    /// Configures the minimum text characters threshold.
    #[must_use]
    pub const fn with_min_text_chars(mut self, min_chars: usize) -> Self {
        self.min_text_chars = min_chars;
        self
    }

    /// Configures the default rasterization DPI.
    #[must_use]
    pub const fn with_default_dpi(mut self, dpi: u32) -> Self {
        self.default_dpi = dpi;
        self
    }

    /// Configures the maximum replacement character ratio tolerance.
    #[must_use]
    pub const fn with_max_replacement_char_ratio(mut self, ratio: f32) -> Self {
        self.max_replacement_char_ratio = ratio;
        self
    }
}
