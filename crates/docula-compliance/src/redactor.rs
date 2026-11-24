use crate::directive::ActionDirective;
use crate::pattern::CensorPattern;
use docula_core::{BoundingBox, SpatialSpan};
use image::{Rgba, RgbaImage};

/// Pure-Rust deterministic compliance engine for pixel blackouts and PII purge.
#[derive(Clone, Debug)]
pub struct ComplianceRedactor {
    patterns: Vec<CensorPattern>,
}

impl Default for ComplianceRedactor {
    fn default() -> Self {
        Self {
            patterns: vec![
                CensorPattern::default_iban(),
                CensorPattern::default_credit_card(),
                CensorPattern::default_ssn(),
            ],
        }
    }
}

impl ComplianceRedactor {
    /// Creates a redactor initialized with custom patterns.
    #[must_use]
    pub fn new(patterns: Vec<CensorPattern>) -> Self {
        Self { patterns }
    }

    /// Appends a new pattern to the redactor's active ruleset.
    pub fn add_pattern(&mut self, pattern: CensorPattern) {
        self.patterns.push(pattern);
    }

    /// Overwrites all pixels inside the bounding box with opaque black `[0, 0, 0, 255]`.
    pub fn apply_blackout(image: &mut RgbaImage, bbox: &BoundingBox) {
        let (width, height) = image.dimensions();
        if width == 0 || height == 0 {
            return;
        }

        let rect = bbox.to_pixel_rect(width, height);
        let x_end = (rect.x + rect.width).min(width);
        let y_end = (rect.y + rect.height).min(height);

        let black = Rgba([0, 0, 0, 255]);
        for y in rect.y..y_end {
            for x in rect.x..x_end {
                image.put_pixel(x, y, black);
            }
        }
    }

    /// Alias for `apply_blackout` matching naming conventions.
    #[inline]
    pub fn apply_pixel_blackout(image: &mut RgbaImage, bbox: &BoundingBox) {
        Self::apply_blackout(image, bbox);
    }

    /// Purges all text spans that overlap with the redacted bounding box.
    pub fn purge_text_spans(spans: &mut Vec<SpatialSpan>, bbox: &BoundingBox) {
        spans.retain(|span| !span.bbox.overlaps(bbox));
    }

    /// Alias for `purge_text_spans` matching naming conventions.
    #[inline]
    pub fn purge_intersecting_spans(spans: &mut Vec<SpatialSpan>, bbox: &BoundingBox) {
        Self::purge_text_spans(spans, bbox);
    }

    /// Redacts a spatial region completely: blacking out image pixels and purging intersecting spans.
    pub fn redact_region(image: &mut RgbaImage, spans: &mut Vec<SpatialSpan>, bbox: &BoundingBox) {
        Self::apply_blackout(image, bbox);
        Self::purge_text_spans(spans, bbox);
    }

    /// Evaluates text spans against all registered regex patterns, blacking out matching regions
    /// on the image and purging the spans from the layout list.
    ///
    /// Returns the number of redacted text spans.
    pub fn auto_censor(&self, image: &mut RgbaImage, spans: &mut Vec<SpatialSpan>) -> usize {
        let mut bboxes_to_censor = Vec::new();
        let mut redacted_count = 0;

        spans.retain(|span| {
            let matched = self
                .patterns
                .iter()
                .any(|p| p.action == ActionDirective::Censor && p.is_match(span.text.trim()));

            if matched {
                bboxes_to_censor.push(span.bbox);
                redacted_count += 1;
                false
            } else {
                true
            }
        });

        for bbox in &bboxes_to_censor {
            Self::apply_blackout(image, bbox);
        }

        for bbox in &bboxes_to_censor {
            Self::purge_text_spans(spans, bbox);
        }

        redacted_count
    }
}
