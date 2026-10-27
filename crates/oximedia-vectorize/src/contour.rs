//! Contour boundary tracing, polyline simplification, and SVG path synthesis.

use image::RgbaImage;

/// Single vectorized contour path.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgVectorPath {
    /// Control and vertex points along the traced contour.
    pub points: Vec<(f32, f32)>,
    /// Whether the contour forms a closed geometric loop.
    pub is_closed: bool,
}

impl SvgVectorPath {
    /// Serializes this vectorized contour into an SVG path specification `d="..."`.
    #[must_use]
    pub fn to_svg_path_data(&self) -> String {
        if self.points.is_empty() {
            return String::new();
        }

        let mut d = String::new();
        for (i, pt) in self.points.iter().enumerate() {
            if i == 0 {
                d.push_str(&format!("M {:.2} {:.2}", pt.0, pt.1));
            } else {
                d.push_str(&format!(" L {:.2} {:.2}", pt.0, pt.1));
            }
        }

        if self.is_closed {
            d.push_str(" Z");
        }

        d
    }
}

/// Vectorized graphic representation containing traced contour paths.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorizedGraphic {
    /// Pixel width of original raster.
    pub width: u32,
    /// Pixel height of original raster.
    pub height: u32,
    /// Vectorized SVG paths.
    pub paths: Vec<SvgVectorPath>,
}

impl VectorizedGraphic {
    /// Synthesizes a standalone SVG document representation.
    #[must_use]
    pub fn to_svg_string(&self) -> String {
        let mut svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}">"#,
            self.width, self.height
        );
        svg.push('\n');

        for path in &self.paths {
            let d = path.to_svg_path_data();
            if !d.is_empty() {
                svg.push_str(&format!(
                    r#"  <path d="{d}" fill="none" stroke="black" stroke-width="1.5" />"#
                ));
                svg.push('\n');
            }
        }

        svg.push_str("</svg>");
        svg
    }
}

/// Operational parameters for vectorization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VectorizeConfig {
    /// Target grid downsampling factor for contour extraction (default: 4.0).
    pub grid_step: f32,
    /// RDP simplification tolerance (epsilon in pixels, default: 2.0).
    pub rdp_epsilon: f32,
}

impl Default for VectorizeConfig {
    fn default() -> Self {
        Self {
            grid_step: 4.0,
            rdp_epsilon: 2.0,
        }
    }
}

/// Vectorizes an RGBA image into an SVG graphic using Otsu thresholding,
/// Moore-Neighbor contour tracing, and RDP simplification.
#[must_use]
pub fn vectorize_rgba_to_svg(img: &RgbaImage, config: VectorizeConfig) -> VectorizedGraphic {
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return VectorizedGraphic {
            width,
            height,
            paths: Vec::new(),
        };
    }

    let mut gray = Vec::with_capacity((width * height) as usize);
    for p in img.pixels() {
        let luma =
            (u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000;
        gray.push(luma as u8);
    }

    let thresh = otsu_threshold(&gray);
    let step = config.grid_step.max(1.0);
    let grid_w = ((width as f32 / step).ceil() as usize).max(1);
    let grid_h = ((height as f32 / step).ceil() as usize).max(1);

    let mut binary_grid = vec![false; grid_w * grid_h];
    for gy in 0..grid_h {
        let py = ((gy as f32 * step) as u32).min(height - 1);
        for gx in 0..grid_w {
            let px = ((gx as f32 * step) as u32).min(width - 1);
            let val = gray[(py * width + px) as usize];
            if val < thresh {
                binary_grid[gy * grid_w + gx] = true;
            }
        }
    }

    let raw_contours =
        trace_moore_neighbor_contours(&binary_grid, grid_w, grid_h, step, step);

    let mut paths = Vec::with_capacity(raw_contours.len());
    for raw in raw_contours {
        if raw.len() < 3 {
            continue;
        }
        let simplified = simplify_rdp(&raw, config.rdp_epsilon);
        if simplified.len() >= 3 {
            paths.push(SvgVectorPath {
                points: simplified,
                is_closed: true,
            });
        }
    }

    VectorizedGraphic {
        width,
        height,
        paths,
    }
}

/// Calculates the optimal global binarization threshold via Otsu's method.
#[must_use]
pub fn otsu_threshold(gray: &[u8]) -> u8 {
    if gray.is_empty() {
        return 128;
    }

    let mut hist = [0u64; 256];
    for &b in gray {
        hist[b as usize] += 1;
    }

    let total = gray.len() as f64;
    let mut sum = 0.0;
    for (t, count) in hist.iter().enumerate() {
        sum += (t as f64) * (*count as f64);
    }

    let mut sum_b = 0.0;
    let mut w_b = 0.0;
    let mut max_variance = 0.0;
    let mut best_thresh = 128u8;

    for (t, count) in hist.iter().enumerate() {
        w_b += *count as f64;
        if w_b == 0.0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0.0 {
            break;
        }

        sum_b += (t as f64) * (*count as f64);
        let m_b = sum_b / w_b;
        let m_f = (sum - sum_b) / w_f;

        let between_variance = w_b * w_f * (m_b - m_f) * (m_b - m_f);
        if between_variance > max_variance {
            max_variance = between_variance;
            best_thresh = t as u8;
        }
    }

    best_thresh
}

/// Traces contours across a downsampled 2D boolean grid using the Moore-Neighbor algorithm.
#[must_use]
pub fn trace_moore_neighbor_contours(
    grid: &[bool],
    gw: usize,
    gh: usize,
    sx: f32,
    sy: f32,
) -> Vec<Vec<(f32, f32)>> {
    let mut visited = vec![false; gw * gh];
    let mut contours = Vec::new();

    let moore_dirs: [(isize, isize); 8] = [
        (0, -1),
        (1, -1),
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
    ];

    for y in 0..gh {
        for x in 0..gw {
            let idx = y * gw + x;
            if grid[idx] && !visited[idx] {
                let is_boundary = x == 0
                    || y == 0
                    || x + 1 == gw
                    || y + 1 == gh
                    || !grid[idx - 1]
                    || !grid[idx + 1]
                    || !grid[idx - gw]
                    || !grid[idx + gw];

                if !is_boundary {
                    continue;
                }

                let mut contour = Vec::new();
                let mut curr_x = x;
                let mut curr_y = y;
                let mut dir_idx = 0usize;

                let max_steps = (gw * gh).min(1000);
                for _ in 0..max_steps {
                    visited[curr_y * gw + curr_x] = true;
                    contour.push((curr_x as f32 * sx, curr_y as f32 * sy));

                    let mut found_next = false;
                    for d in 0..8 {
                        let check_dir = (dir_idx + d) % 8;
                        let nx = curr_x as isize + moore_dirs[check_dir].0;
                        let ny = curr_y as isize + moore_dirs[check_dir].1;

                        if nx >= 0 && nx < gw as isize && ny >= 0 && ny < gh as isize {
                            let n_idx = (ny as usize) * gw + (nx as usize);
                            if grid[n_idx] {
                                curr_x = nx as usize;
                                curr_y = ny as usize;
                                dir_idx = (check_dir + 6) % 8;
                                found_next = true;
                                break;
                            }
                        }
                    }

                    if !found_next || (curr_x == x && curr_y == y && contour.len() > 2) {
                        break;
                    }
                }

                if contour.len() >= 3 {
                    contours.push(contour);
                }
            }
        }
    }

    contours
}

/// Simplifies a polygonal chain of 2D points using the Ramer-Douglas-Peucker (RDP) algorithm.
#[must_use]
pub fn simplify_rdp(points: &[(f32, f32)], epsilon: f32) -> Vec<(f32, f32)> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let mut max_dist = 0.0f32;
    let mut index = 0usize;
    let start = points[0];
    let end = points[points.len() - 1];

    for (i, &pt) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        let dist = perpendicular_distance(pt, start, end);
        if dist > max_dist {
            max_dist = dist;
            index = i;
        }
    }

    if max_dist > epsilon {
        let mut rec1 = simplify_rdp(&points[0..=index], epsilon);
        let rec2 = simplify_rdp(&points[index..], epsilon);
        rec1.pop();
        rec1.extend(rec2);
        rec1
    } else {
        vec![start, end]
    }
}

/// Computes perpendicular distance from point `pt` to the line segment between `l1` and `l2`.
#[must_use]
pub fn perpendicular_distance(pt: (f32, f32), l1: (f32, f32), l2: (f32, f32)) -> f32 {
    let dx = l2.0 - l1.0;
    let dy = l2.1 - l1.1;
    let line_len_sq = dx * dx + dy * dy;

    if line_len_sq < 1e-6 {
        let px = pt.0 - l1.0;
        let py = pt.1 - l1.1;
        return (px * px + py * py).sqrt();
    }

    let num = ((l2.0 - l1.0) * (l1.1 - pt.1) - (l1.0 - pt.0) * (l2.1 - l1.1)).abs();
    num / line_len_sq.sqrt()
}
