use docula_core::{BoundingBox, SpatialSpan};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// Cohesive horizontal line of text constructed from grouped spatial spans.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextLine {
    /// Ordered constituent text spans from left to right.
    pub spans: Vec<SpatialSpan>,
    /// Axis-aligned bounding box enclosing all spans in the line.
    pub bbox: BoundingBox,
    /// Concatenated text representation of the line.
    pub text: String,
}

impl TextLine {
    /// Constructs a `TextLine` from a non-empty sequence of sorted spans.
    #[must_use]
    pub fn from_spans(spans: Vec<SpatialSpan>) -> Option<Self> {
        if spans.is_empty() {
            return None;
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        let mut line_text = String::new();

        for (idx, span) in spans.iter().enumerate() {
            min_x = min_x.min(span.bbox.top_left.x);
            min_y = min_y.min(span.bbox.top_left.y);
            max_x = max_x.max(span.bbox.bottom_right.x);
            max_y = max_y.max(span.bbox.bottom_right.y);

            if idx > 0 {
                // If there's a horizontal gap, separate with spaces
                let prev_right = spans[idx - 1].bbox.bottom_right.x;
                let gap = span.bbox.top_left.x - prev_right;
                if gap > 0.05 {
                    line_text.push_str("    "); // Tabular column gap
                } else {
                    line_text.push(' ');
                }
            }
            line_text.push_str(span.text.trim());
        }

        let bbox = BoundingBox::from_coords_clamped(min_x, min_y, max_x, max_y);

        Some(Self {
            spans,
            bbox,
            text: line_text,
        })
    }

    /// Returns the baseline vertical center of the text line.
    #[must_use]
    pub fn vertical_center(&self) -> f32 {
        (self.bbox.top_left.y + self.bbox.bottom_right.y) * 0.5
    }

    /// Checks if a span shares vertical baseline proximity with this line.
    #[must_use]
    pub fn matches_vertical_baseline(&self, span: &SpatialSpan, tolerance: f32) -> bool {
        let span_mid_y = (span.bbox.top_left.y + span.bbox.bottom_right.y) * 0.5;
        let line_mid_y = self.vertical_center();
        let height = self.bbox.height().max(span.bbox.height());

        (span_mid_y - line_mid_y).abs() <= (height * tolerance).max(0.015)
    }
}

/// 2D spatial layout map preserving reading order, columns, and tabular alignments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpatialLayoutMap {
    /// Ordered horizontal lines from top to bottom.
    pub lines: Vec<TextLine>,
    /// Page dimensions `(width, height)` in pixels or points.
    pub page_dimensions: (u32, u32),
}

impl SpatialLayoutMap {
    /// Constructs a `SpatialLayoutMap` by clustering unordered spans into reading order.
    #[must_use]
    pub fn from_spans(mut spans: Vec<SpatialSpan>, page_dimensions: (u32, u32)) -> Self {
        if spans.is_empty() {
            return Self {
                lines: Vec::new(),
                page_dimensions,
            };
        }

        // Sort spans primarily by vertical Y, secondarily by horizontal X
        spans.sort_by(|a, b| {
            a.bbox
                .top_left
                .y
                .partial_cmp(&b.bbox.top_left.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.bbox
                        .top_left
                        .x
                        .partial_cmp(&b.bbox.top_left.x)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });

        // Cluster spans into horizontal lines
        let mut clustered_lines: Vec<Vec<SpatialSpan>> = Vec::new();

        for span in spans {
            let span_mid_y = (span.bbox.top_left.y + span.bbox.bottom_right.y) * 0.5;
            let mut added_to_line = false;

            for line_spans in &mut clustered_lines {
                if let Some(first) = line_spans.first() {
                    let first_mid_y = (first.bbox.top_left.y + first.bbox.bottom_right.y) * 0.5;
                    let line_height = first.bbox.height().max(span.bbox.height());

                    // Vertical overlap tolerance
                    if (span_mid_y - first_mid_y).abs() <= line_height * 0.65 {
                        line_spans.push(span.clone());
                        added_to_line = true;
                        break;
                    }
                }
            }

            if !added_to_line {
                clustered_lines.push(vec![span]);
            }
        }

        // For each line, sort constituent spans horizontally by X and construct TextLine
        let mut lines = Vec::with_capacity(clustered_lines.len());
        for mut line_spans in clustered_lines {
            line_spans.sort_by(|a, b| {
                a.bbox
                    .top_left
                    .x
                    .partial_cmp(&b.bbox.top_left.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            if let Some(text_line) = TextLine::from_spans(line_spans) {
                lines.push(text_line);
            }
        }

        // Sort lines top to bottom by their bounding box top_left.y
        lines.sort_by(|a, b| {
            a.bbox
                .top_left
                .y
                .partial_cmp(&b.bbox.top_left.y)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Self {
            lines,
            page_dimensions,
        }
    }

    /// Serializes the spatial layout into a clean plain text representation.
    #[must_use]
    pub fn to_plain_text(&self) -> String {
        let mut output = String::new();
        for line in &self.lines {
            output.push_str(&line.text);
            output.push('\n');
        }
        output
    }

    /// Serializes the layout with normalized coordinate timestamps suitable for LLM spatial reasoning.
    ///
    /// Format: `[y0, x0] Content`
    #[must_use]
    pub fn to_coordinate_formatted_text(&self) -> String {
        let mut output = String::new();
        let (w, h) = self.page_dimensions;
        let w_f = if w == 0 { 1000.0 } else { w as f32 };
        let h_f = if h == 0 { 1000.0 } else { h as f32 };

        for line in &self.lines {
            let px_x = (line.bbox.top_left.x * w_f).round() as u32;
            let px_y = (line.bbox.top_left.y * h_f).round() as u32;

            let _ = writeln!(output, "[{px_y:04}, {px_x:04}] {}", line.text);
        }

        output
    }

    /// Total number of reconstructed lines in the layout.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Checks if the layout contains any text.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}
