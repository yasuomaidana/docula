//! Visual region extraction, foreground isolation, and vectorization engine.
//!
//! Provides [`VisualRegion`], [`VectorizedRegion`], and high-level visual features:
//! - **Mask only**: Pure binary segmentation mask of high-contrast foreground strokes.
//! - **Isolate foreground**: Isolated strokes in pure black with transparent background.
//! - **Grayscale**: Normalized luminance conversion eliminating color hues while preserving stroke shading.
//! - **Vectorize**: Algorithmic contour tracing and SVG vector path synthesis.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use docula_core::{BoundingBox, CropFormat, ImageCrop, Point, Polygon};
use image::{ImageEncoder, Rgba, RgbaImage};

use crate::error::PreprocessError;

/// Value object representing a cropped visual region-of-interest (ROI) and its spatial context.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualRegion {
    /// Normalized bounding box $[0.0, 1.0]$ in document coordinate space.
    pub bbox: BoundingBox,
    /// Raw cropped RGBA image pixmap.
    pub image: RgbaImage,
}

/// Vectorized representation of a visual region containing SVG markup and polygons.
#[derive(Clone, Debug, PartialEq)]
pub struct VectorizedRegion {
    /// Normalized bounding box of the vectorized region.
    pub bbox: BoundingBox,
    /// Pixel width of the source region.
    pub width: u32,
    /// Pixel height of the source region.
    pub height: u32,
    /// Polygonal contours forming the vector outlines.
    pub polygons: Vec<Polygon>,
    /// Complete standalone SVG markup string.
    pub svg: String,
    /// SVG path `d` attribute data string.
    pub path_data: String,
}

impl VectorizedRegion {
    /// Returns the SVG markup string.
    #[inline]
    #[must_use]
    pub fn to_svg(&self) -> &str {
        &self.svg
    }

    /// Returns the number of traced vector polygons.
    #[inline]
    #[must_use]
    pub fn polygon_count(&self) -> usize {
        self.polygons.len()
    }

    /// Returns a slice of the traced vector polygons.
    #[inline]
    #[must_use]
    pub fn polygons(&self) -> &[Polygon] {
        &self.polygons
    }

    /// Returns the SVG path `d` data string.
    #[inline]
    #[must_use]
    pub fn path_data(&self) -> &str {
        &self.path_data
    }
}

impl VisualRegion {
    /// Constructs a new `VisualRegion` from an RGBA pixmap and bounding box.
    #[must_use]
    pub fn new(image: RgbaImage, bbox: BoundingBox) -> Self {
        Self { bbox, image }
    }

    /// Extracts a visual region from an arbitrary source image using a normalized bounding box.
    #[must_use]
    pub fn extract_from_image(source: &RgbaImage, bbox: &BoundingBox) -> Self {
        let (w, h) = source.dimensions();
        let rect = bbox.to_pixel_rect(w, h);
        let cropped = image::imageops::crop_imm(source, rect.x, rect.y, rect.width, rect.height);
        Self::new(cropped.to_image(), *bbox)
    }

    /// Reference to the raw RGBA image pixmap.
    #[inline]
    #[must_use]
    pub fn raw_image(&self) -> &RgbaImage {
        &self.image
    }

    /// Bounding box of the visual region in source document coordinates.
    #[inline]
    #[must_use]
    pub fn bbox(&self) -> &BoundingBox {
        &self.bbox
    }

    /// Pixel dimensions `(width, height)` of the region crop.
    #[inline]
    #[must_use]
    pub fn dimensions(&self) -> (u32, u32) {
        self.image.dimensions()
    }

    /// Pixel width of the region crop.
    #[inline]
    #[must_use]
    pub fn width(&self) -> u32 {
        self.image.width()
    }

    /// Pixel height of the region crop.
    #[inline]
    #[must_use]
    pub fn height(&self) -> u32 {
        self.image.height()
    }

    /// Encodes the visual region into an [`ImageCrop`] value object.
    pub fn to_image_crop(&self, format: CropFormat) -> Result<ImageCrop, PreprocessError> {
        let mut bytes = Vec::new();
        match format {
            CropFormat::Png => {
                let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
                encoder
                    .write_image(
                        self.image.as_raw(),
                        self.width(),
                        self.height(),
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|e| PreprocessError::ImageEncodingError {
                        reason: e.to_string(),
                    })?;
            }
            CropFormat::RawRgba => {
                bytes.extend_from_slice(self.image.as_raw());
            }
            _ => {
                let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
                encoder
                    .write_image(
                        self.image.as_raw(),
                        self.width(),
                        self.height(),
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|e| PreprocessError::ImageEncodingError {
                        reason: e.to_string(),
                    })?;
            }
        }

        let encoded_data = BASE64_STANDARD.encode(&bytes);
        ImageCrop::new(
            0,
            self.bbox,
            self.width(),
            self.height(),
            encoded_data,
            format.mime_type(),
        )
        .map_err(PreprocessError::Crop)
    }

    /// Generates a binary segmentation mask where foreground strokes are opaque white `[255, 255, 255, 255]`
    /// and background pixels are opaque black `[0, 0, 0, 255]`.
    #[must_use]
    pub fn mask_only(&self) -> RgbaImage {
        self.mask_with_colors(Rgba([255, 255, 255, 255]), Rgba([0, 0, 0, 255]))
    }

    /// Generates a binary segmentation mask with dark foreground strokes on a clean white background.
    #[must_use]
    pub fn mask_black_on_white(&self) -> RgbaImage {
        self.mask_with_colors(Rgba([0, 0, 0, 255]), Rgba([255, 255, 255, 255]))
    }

    /// Generates a binary segmentation mask with custom foreground and background colors.
    #[must_use]
    pub fn mask_with_colors(&self, fg_color: Rgba<u8>, bg_color: Rgba<u8>) -> RgbaImage {
        let threshold = self.compute_optimal_threshold();
        let (w, h) = self.dimensions();
        let mut mask = RgbaImage::new(w, h);

        for y in 0..h {
            for x in 0..w {
                let p = self.image.get_pixel(x, y);
                let luma = pixel_luma(p);
                if luma <= threshold && p[3] > 64 {
                    mask.put_pixel(x, y, fg_color);
                } else {
                    mask.put_pixel(x, y, bg_color);
                }
            }
        }
        mask
    }

    /// Isolates foreground stroke pixels in pure monochrome black `[0, 0, 0, alpha]`,
    /// setting all background pixels to fully transparent `[0, 0, 0, 0]`.
    ///
    /// Strips color hues and document backgrounds while preserving stroke anti-aliasing.
    #[must_use]
    pub fn isolate_foreground(&self) -> RgbaImage {
        let threshold = self.compute_optimal_threshold();
        let (w, h) = self.dimensions();
        let mut result = RgbaImage::new(w, h);

        for y in 0..h {
            for x in 0..w {
                let p = self.image.get_pixel(x, y);
                let luma = pixel_luma(p);
                if luma <= threshold && p[3] > 64 {
                    // Compute smooth alpha preserving anti-aliased boundaries
                    let alpha = if threshold > 0 {
                        let ratio = (threshold - luma) as f32 / threshold as f32;
                        ((ratio * 180.0 + 75.0).clamp(0.0, 255.0) as u8).min(p[3])
                    } else {
                        p[3]
                    };
                    result.put_pixel(x, y, Rgba([0, 0, 0, alpha]));
                } else {
                    result.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                }
            }
        }
        result
    }

    /// Converts the entire cropped visual region into pure grayscale,
    /// setting each channel $R = G = B = \text{Luminance}$.
    ///
    /// Removes all chrominance / color tint while preserving original shading and background texture.
    #[must_use]
    pub fn grayscale(&self) -> RgbaImage {
        let (w, h) = self.dimensions();
        let mut gray = RgbaImage::new(w, h);

        for y in 0..h {
            for x in 0..w {
                let p = self.image.get_pixel(x, y);
                let luma = pixel_luma(p);
                gray.put_pixel(x, y, Rgba([luma, luma, luma, p[3]]));
            }
        }
        gray
    }

    /// Vectorizes high-contrast foreground strokes into geometric vector contours ([`Polygon`])
    /// and standard SVG `<svg>...</svg>` markup.
    #[must_use]
    pub fn vectorize(&self) -> VectorizedRegion {
        let threshold = self.compute_optimal_threshold();
        let (w, h) = self.dimensions();

        // 1. Build 2D boolean foreground grid
        let mut grid = vec![vec![false; w as usize]; h as usize];
        for y in 0..h {
            for x in 0..w {
                let p = self.image.get_pixel(x, y);
                let luma = pixel_luma(p);
                if luma <= threshold && p[3] > 64 {
                    grid[y as usize][x as usize] = true;
                }
            }
        }

        // 2. Extract boundary contours
        let contours = trace_grid_contours(&grid, w, h);

        // 3. Convert simplified contours to Polygons and SVG paths
        let mut polygons = Vec::new();
        let mut path_fragments = Vec::new();

        for contour in &contours {
            if contour.len() < 3 {
                continue;
            }

            // SVG path fragment
            let mut frag = format!("M {:.1} {:.1}", contour[0].0, contour[0].1);
            for pt in &contour[1..] {
                frag.push_str(&format!(" L {:.1} {:.1}", pt.0, pt.1));
            }
            frag.push_str(" Z");
            path_fragments.push(frag);

            // Polygon in normalized [0.0, 1.0] coordinates within the crop
            let vertices: Vec<Point> = contour
                .iter()
                .map(|&(px, py)| {
                    Point::new_unchecked(
                        (px / w as f32).clamp(0.0, 1.0),
                        (py / h as f32).clamp(0.0, 1.0),
                    )
                })
                .collect();

            if let Ok(poly) = Polygon::new(vertices) {
                polygons.push(poly);
            }
        }

        let path_data = path_fragments.join(" ");
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}"><path d="{path_data}" fill="#000000" fill-rule="evenodd"/></svg>"##
        );

        VectorizedRegion {
            bbox: self.bbox,
            width: w,
            height: h,
            polygons,
            svg,
            path_data,
        }
    }

    /// Computes the Otsu clustering threshold separating dark foreground strokes from background.
    fn compute_optimal_threshold(&self) -> u8 {
        let (w, h) = self.dimensions();
        if w == 0 || h == 0 {
            return 128;
        }

        let mut histogram = [0u32; 256];
        let mut total_pixels = 0u32;

        for y in 0..h {
            for x in 0..w {
                let p = self.image.get_pixel(x, y);
                if p[3] > 64 {
                    let luma = pixel_luma(p);
                    histogram[luma as usize] += 1;
                    total_pixels += 1;
                }
            }
        }

        if total_pixels == 0 {
            return 128;
        }

        let total = total_pixels as f64;
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

        // Clamp to a reasonable threshold range [80, 180] for document scans
        threshold.clamp(80, 180)
    }
}

/// Computes standard perceived luminance ($Y = 0.299R + 0.587G + 0.114B$) for an RGBA pixel.
#[inline]
fn pixel_luma(p: &Rgba<u8>) -> u8 {
    let l = (u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000;
    l as u8
}

/// Traces boundary contours of connected components in a 2D boolean grid.
fn trace_grid_contours(grid: &[Vec<bool>], width: u32, height: u32) -> Vec<Vec<(f32, f32)>> {
    let w = width as usize;
    let h = height as usize;
    let mut visited = vec![vec![false; w]; h];
    let mut contours = Vec::new();

    // 8-connected Moore neighborhood offsets in clockwise order
    let neighbors: [(i32, i32); 8] = [
        (1, 0),   // 0: East
        (1, 1),   // 1: South-East
        (0, 1),   // 2: South
        (-1, 1),  // 3: South-West
        (-1, 0),  // 4: West
        (-1, -1), // 5: North-West
        (0, -1),  // 6: North
        (1, -1),  // 7: North-East
    ];

    for y in 0..h {
        for x in 0..w {
            if !grid[y][x] || visited[y][x] {
                continue;
            }

            // First, explore connected component
            let mut component_pixels = Vec::new();
            let mut queue = vec![(x, y)];
            visited[y][x] = true;

            while let Some((cx, cy)) = queue.pop() {
                component_pixels.push((cx, cy));

                for &(dx, dy) in &neighbors {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let ux = nx as usize;
                        let uy = ny as usize;
                        if grid[uy][ux] && !visited[uy][ux] {
                            visited[uy][ux] = true;
                            queue.push((ux, uy));
                        }
                    }
                }
            }

            // Filter tiny noise specks (< 4 pixels)
            if component_pixels.len() < 4 {
                continue;
            }

            // Find top-left-most boundary pixel in this component
            let start = component_pixels
                .iter()
                .min_by_key(|&&(px, py)| (py, px))
                .copied()
                .unwrap();

            // Trace outer boundary loop using Moore-Neighbor tracing
            let mut loop_points = Vec::new();
            let mut curr = start;
            let mut check_dir = 7; // Start looking North-East
            let max_steps = (w * h).min(2000);

            for _ in 0..max_steps {
                loop_points.push((curr.0 as f32, curr.1 as f32));

                let mut found_next = false;
                for step in 0..8 {
                    let dir_idx = (check_dir + step) % 8;
                    let (dx, dy) = neighbors[dir_idx];
                    let nx = curr.0 as i32 + dx;
                    let ny = curr.1 as i32 + dy;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let ux = nx as usize;
                        let uy = ny as usize;
                        if grid[uy][ux] {
                            curr = (ux, uy);
                            // Next search starts opposite to incoming direction
                            check_dir = (dir_idx + 5) % 8;
                            found_next = true;
                            break;
                        }
                    }
                }

                if !found_next || curr == start {
                    break;
                }
            }

            if loop_points.len() >= 3 {
                let simplified = simplify_rdp(&loop_points, 1.2);
                if simplified.len() >= 3 {
                    contours.push(simplified);
                }
            }
        }
    }

    contours
}

/// Ramer-Douglas-Peucker (RDP) polyline simplification algorithm.
fn simplify_rdp(points: &[(f32, f32)], epsilon: f32) -> Vec<(f32, f32)> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let mut max_dist = 0.0f32;
    let mut index = 0;
    let start = points[0];
    let end = *points.last().unwrap();

    let line_dx = end.0 - start.0;
    let line_dy = end.1 - start.1;
    let line_len_sq = line_dx * line_dx + line_dy * line_dy;

    for (i, &p) in points.iter().enumerate().take(points.len() - 1).skip(1) {
        let dist = if line_len_sq < 1e-6 {
            let dx = p.0 - start.0;
            let dy = p.1 - start.1;
            (dx * dx + dy * dy).sqrt()
        } else {
            let num = ((p.0 - start.0) * line_dy - (p.1 - start.1) * line_dx).abs();
            num / line_len_sq.sqrt()
        };

        if dist > max_dist {
            max_dist = dist;
            index = i;
        }
    }

    if max_dist > epsilon {
        let mut left = simplify_rdp(&points[..=index], epsilon);
        let right = simplify_rdp(&points[index..], epsilon);
        left.pop();
        left.extend(right);
        left
    } else {
        vec![start, end]
    }
}
