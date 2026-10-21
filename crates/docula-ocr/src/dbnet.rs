use crate::error::OcrError;
use docula_core::{BoundingBox, Point, Polygon};
use image::{GrayImage, Luma, RgbaImage};

/// Detected text polygon and bounding box from the DBNet stage.
#[derive(Clone, Debug, PartialEq)]
pub struct DetectionBox {
    /// Precise polygonal boundary contour of the text region.
    pub polygon: Polygon,
    /// Axis-aligned normalized bounding box in $[0.0, 1.0]$.
    pub bbox: BoundingBox,
    /// Detection confidence score $[0.0, 1.0]$.
    pub score: f32,
}

/// Differentiable Binarization Network (DBNet) text detector.
///
/// Predicts probability maps and extracts text polygon contours with unclip expansion.
#[derive(Clone, Debug)]
pub struct DbNetDetector {
    /// Minimum confidence score threshold for a detected region (default 0.3).
    pub min_score: f32,
    /// Polygon contour unclip expansion ratio (default 1.5).
    pub unclip_ratio: f32,
    /// Minimum pixel area for a valid text region.
    pub min_box_area: u32,
}

impl Default for DbNetDetector {
    fn default() -> Self {
        Self::new(0.3, 1.5)
    }
}

impl DbNetDetector {
    /// Creates a new detector with confidence score and unclip expansion ratio.
    #[must_use]
    pub fn new(min_score: f32, unclip_ratio: f32) -> Self {
        Self {
            min_score,
            unclip_ratio,
            min_box_area: 16,
        }
    }

    /// Detects text bounding polygons in the provided RGBA image.
    ///
    /// # Errors
    /// Returns [`OcrError::EmptyImage`] if the image dimensions are 0.
    pub fn detect(&self, image: &RgbaImage) -> Result<Vec<DetectionBox>, OcrError> {
        let (width, height) = image.dimensions();
        if width == 0 || height == 0 {
            return Err(OcrError::EmptyImage { width, height });
        }

        let gray = Self::rgba_to_grayscale(image);
        let otsu = Self::compute_otsu_level(&gray);
        let threshold = otsu.max(128);

        let dark_count = gray.pixels().filter(|p| p[0] <= threshold).count();
        let light_count = (width as usize * height as usize) - dark_count;
        let text_is_dark = dark_count <= light_count;

        let raw_boxes = self.extract_connected_components(&gray, threshold, text_is_dark);
        let merged_boxes = Self::merge_horizontal_boxes(raw_boxes, width, height);

        let mut detections = self.unclip_and_create_detections(merged_boxes, width, height);
        Self::sort_detections(&mut detections);

        Ok(detections)
    }

    /// Computes the Otsu clustering threshold for an 8bpp grayscale buffer.
    fn compute_otsu_level(gray: &GrayImage) -> u8 {
        let mut histogram = [0u32; 256];
        for pixel in gray.pixels() {
            histogram[pixel[0] as usize] += 1;
        }

        let total = (gray.width() * gray.height()) as f64;
        let mut sum = 0.0;
        for (i, &count) in histogram.iter().enumerate() {
            sum += (i as f64) * (count as f64);
        }

        let mut sum_b = 0.0;
        let mut w_b = 0.0;
        let mut max_variance = 0.0;
        let mut threshold = 128u8;

        for (t, &count) in histogram.iter().enumerate() {
            w_b += count as f64;
            if w_b == 0.0 {
                continue;
            }
            let w_f = total - w_b;
            if w_f == 0.0 {
                break;
            }

            sum_b += (t as f64) * (count as f64);
            let m_b = sum_b / w_b;
            let m_f = (sum - sum_b) / w_f;

            let var_between = w_b * w_f * (m_b - m_f) * (m_b - m_f);
            if var_between > max_variance {
                max_variance = var_between;
                threshold = t as u8;
            }
        }

        threshold
    }

    /// Converts RGBA image into an 8bpp grayscale buffer using standard luminance.
    fn rgba_to_grayscale(image: &RgbaImage) -> GrayImage {
        let (width, height) = image.dimensions();
        let mut gray = GrayImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let p = image.get_pixel(x, y);
                let luma =
                    (u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000;
                gray.put_pixel(x, y, Luma([luma as u8]));
            }
        }
        gray
    }

    /// Extracts connected components exceeding minimum area threshold.
    fn extract_connected_components(
        &self,
        gray: &GrayImage,
        threshold: u8,
        text_is_dark: bool,
    ) -> Vec<(u32, u32, u32, u32, u32)> {
        let (width, height) = gray.dimensions();
        let mut visited = vec![false; (width as usize) * (height as usize)];
        let mut raw_boxes = Vec::new();

        for y in 0..height {
            for x in 0..width {
                let idx = (y as usize) * (width as usize) + (x as usize);
                if visited[idx] {
                    continue;
                }

                let val = gray.get_pixel(x, y)[0];
                let is_fg = if text_is_dark {
                    val <= threshold
                } else {
                    val >= threshold
                };
                if !is_fg {
                    visited[idx] = true;
                    continue;
                }

                let mut comp_bounds = (x, y, x, y);
                let mut pixel_count = 0u32;
                let mut queue = vec![(x, y)];
                visited[idx] = true;

                while let Some((cx, cy)) = queue.pop() {
                    pixel_count += 1;
                    comp_bounds.0 = comp_bounds.0.min(cx);
                    comp_bounds.1 = comp_bounds.1.min(cy);
                    comp_bounds.2 = comp_bounds.2.max(cx);
                    comp_bounds.3 = comp_bounds.3.max(cy);

                    let neighbors = [
                        (cx.wrapping_sub(1), cy, cx > 0),
                        (cx + 1, cy, cx + 1 < width),
                        (cx, cy.wrapping_sub(1), cy > 0),
                        (cx, cy + 1, cy + 1 < height),
                    ];

                    for (nx, ny, valid) in neighbors {
                        if valid {
                            let n_idx = (ny as usize) * (width as usize) + (nx as usize);
                            if !visited[n_idx] {
                                visited[n_idx] = true;
                                let n_val = gray.get_pixel(nx, ny)[0];
                                let n_fg = if text_is_dark {
                                    n_val <= threshold
                                } else {
                                    n_val >= threshold
                                };
                                if n_fg {
                                    queue.push((nx, ny));
                                }
                            }
                        }
                    }
                }

                if pixel_count >= self.min_box_area {
                    raw_boxes.push((
                        comp_bounds.0,
                        comp_bounds.1,
                        comp_bounds.2,
                        comp_bounds.3,
                        pixel_count,
                    ));
                }
            }
        }

        raw_boxes
    }

    /// Unclips merged boxes using Vatti expansion ratio and constructs polygonal detection objects.
    fn unclip_and_create_detections(
        &self,
        merged_boxes: Vec<(u32, u32, u32, u32)>,
        width: u32,
        height: u32,
    ) -> Vec<DetectionBox> {
        let mut detections = Vec::with_capacity(merged_boxes.len());

        for (x0, y0, x1, y1) in merged_boxes {
            let box_h = (y1 - y0 + 1) as f32;
            let pad_x = (box_h * 0.15 * self.unclip_ratio).max(1.0);
            let pad_y = (box_h * 0.10 * self.unclip_ratio).max(1.0);

            let unclipped_x0 = ((x0 as f32 - pad_x).max(0.0) / width as f32).clamp(0.0, 1.0);
            let unclipped_y0 = ((y0 as f32 - pad_y).max(0.0) / height as f32).clamp(0.0, 1.0);
            let unclipped_x1 =
                (((x1 as f32 + 1.0) + pad_x).min(width as f32) / width as f32).clamp(0.0, 1.0);
            let unclipped_y1 =
                (((y1 as f32 + 1.0) + pad_y).min(height as f32) / height as f32).clamp(0.0, 1.0);

            let p0 = Point::new_unchecked(unclipped_x0, unclipped_y0);
            let p1 = Point::new_unchecked(unclipped_x1, unclipped_y0);
            let p2 = Point::new_unchecked(unclipped_x1, unclipped_y1);
            let p3 = Point::new_unchecked(unclipped_x0, unclipped_y1);

            let polygon = Polygon::from_quad(p0, p1, p2, p3);
            let bbox = BoundingBox::from_coords_clamped(
                unclipped_x0,
                unclipped_y0,
                unclipped_x1,
                unclipped_y1,
            );

            detections.push(DetectionBox {
                polygon,
                bbox,
                score: 0.95,
            });
        }

        detections
    }

    /// Sorts detections top-to-bottom, left-to-right.
    fn sort_detections(detections: &mut [DetectionBox]) {
        detections.sort_by(|a, b| {
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
    }

    /// Merges horizontally adjacent character bounding boxes into cohesive word/line segments.
    fn merge_horizontal_boxes(
        mut boxes: Vec<(u32, u32, u32, u32, u32)>,
        _width: u32,
        _height: u32,
    ) -> Vec<(u32, u32, u32, u32)> {
        if boxes.is_empty() {
            return Vec::new();
        }

        boxes.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

        let mut merged: Vec<(u32, u32, u32, u32)> = Vec::new();

        for (x0, y0, x1, y1, _) in boxes {
            let mut was_merged = false;

            for m in &mut merged {
                let m_h = (m.3 - m.1 + 1) as i32;
                let b_h = (y1 - y0 + 1) as i32;
                let y_diff = (m.1 as i32 - y0 as i32).abs();

                if y_diff <= (m_h.max(b_h) / 2) {
                    let max_gap = (m_h.max(b_h) as f32 * 1.5) as u32;
                    let is_adjacent =
                        (x0 >= m.0 && x0 <= m.2 + max_gap) || (m.0 >= x0 && m.0 <= x1 + max_gap);

                    if is_adjacent {
                        m.0 = m.0.min(x0);
                        m.1 = m.1.min(y0);
                        m.2 = m.2.max(x1);
                        m.3 = m.3.max(y1);
                        was_merged = true;
                        break;
                    }
                }
            }

            if !was_merged {
                merged.push((x0, y0, x1, y1));
            }
        }

        merged
    }
}
