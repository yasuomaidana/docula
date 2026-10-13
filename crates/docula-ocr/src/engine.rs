use crate::crnn::CrnnRecognizer;
use crate::dbnet::DbNetDetector;
use crate::error::OcrError;
use docula_core::SpatialSpan;
use image::RgbaImage;

/// Strategy trait for optical character recognition engines.
pub trait OcrEngine: Send + Sync {
    /// Extracts spatial text spans and normalized coordinates from an image.
    ///
    /// # Errors
    /// Returns [`OcrError`] if image detection or character recognition fails.
    fn extract_spans(&self, image: &RgbaImage) -> Result<Vec<SpatialSpan>, OcrError>;
}

/// Neural multi-lingual OCR pipeline combining DBNet detection and CRNN recognition.
pub struct NeuralOcrEngine {
    detector: DbNetDetector,
    recognizer: CrnnRecognizer,
}

impl Default for NeuralOcrEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl NeuralOcrEngine {
    /// Creates a new `NeuralOcrEngine` with default detection and recognition settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            detector: DbNetDetector::default(),
            recognizer: CrnnRecognizer::new(),
        }
    }

    /// Configures custom DBNet detection parameters.
    #[must_use]
    pub fn with_detector(mut self, detector: DbNetDetector) -> Self {
        self.detector = detector;
        self
    }

    /// Returns a reference to the internal CRNN recognizer.
    #[must_use]
    pub const fn recognizer(&self) -> &CrnnRecognizer {
        &self.recognizer
    }

    /// Returns a reference to the internal DBNet detector.
    #[must_use]
    pub const fn detector(&self) -> &DbNetDetector {
        &self.detector
    }
}

impl OcrEngine for NeuralOcrEngine {
    fn extract_spans(&self, image: &RgbaImage) -> Result<Vec<SpatialSpan>, OcrError> {
        let (width, height) = image.dimensions();
        if width == 0 || height == 0 {
            return Err(OcrError::EmptyImage { width, height });
        }

        // 1. Text detection via DBNet
        let boxes = self.detector.detect(image)?;

        // 2. Parallel character recognition via CRNN with Rayon
        self.recognizer.recognize_batch(image, &boxes, 0)
    }
}

/// Deterministic mock OCR engine for hermetic test execution.
#[derive(Clone, Debug, Default)]
pub struct MockOcrEngine {
    spans: Vec<SpatialSpan>,
}

impl MockOcrEngine {
    /// Creates an empty `MockOcrEngine`.
    #[must_use]
    pub fn new() -> Self {
        Self { spans: Vec::new() }
    }

    /// Constructs a mock engine pre-loaded with static spatial spans.
    #[must_use]
    pub fn with_spans(spans: Vec<SpatialSpan>) -> Self {
        Self { spans }
    }

    /// Appends a span to the mock engine in place.
    pub fn add_span(&mut self, span: SpatialSpan) -> &mut Self {
        self.spans.push(span);
        self
    }
}

impl OcrEngine for MockOcrEngine {
    fn extract_spans(&self, image: &RgbaImage) -> Result<Vec<SpatialSpan>, OcrError> {
        let (width, height) = image.dimensions();
        if width == 0 || height == 0 {
            return Err(OcrError::EmptyImage { width, height });
        }

        Ok(self.spans.clone())
    }
}
