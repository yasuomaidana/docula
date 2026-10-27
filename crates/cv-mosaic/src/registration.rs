//! Axis-parameterized ZMAD strip overlap discovery and ECC drift estimation.

use image::RgbaImage;
use oximedia_cv::registration::TransformationType;
use oximedia_cv::registration::ecc::register_ecc;

/// Orientation axis for 2D overlap registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlapAxis {
    /// Vertical strip sequence (top image overlapping bottom image).
    Vertical,
    /// Horizontal strip sequence (left image overlapping right image).
    Horizontal,
}

/// Detects overlap extent between two consecutive image strips using Zero-Mean Absolute Difference (ZMAD).
#[must_use]
pub fn detect_strip_overlap_zmad(
    img_a: &RgbaImage,
    img_b: &RgbaImage,
    axis: OverlapAxis,
    min_ratio: f64,
    max_ratio: f64,
) -> (u32, f64) {
    let (w_a, h_a) = img_a.dimensions();
    let (w_b, h_b) = img_b.dimensions();

    let (extent_a, extent_b, cross_dim) = match axis {
        OverlapAxis::Vertical => (h_a, h_b, w_a.min(w_b)),
        OverlapAxis::Horizontal => (w_a, w_b, h_a.min(h_b)),
    };

    let min_overlap = ((extent_a as f64 * min_ratio) as u32).max(12);
    let max_overlap = ((extent_a as f64 * max_ratio) as u32)
        .min(extent_a.saturating_sub(1))
        .min(extent_b.saturating_sub(1));

    if max_overlap <= min_overlap || cross_dim == 0 {
        return (min_overlap, f64::MAX);
    }

    let mut best_overlap = min_overlap;
    let mut min_avg_diff = f64::MAX;

    for cand in min_overlap..=max_overlap {
        let mut sum1 = 0u64;
        let mut sum2 = 0u64;
        let mut count = 0u64;
        let step_c = (cand / 15).max(1);

        for c in (0..cand).step_by(step_c as usize) {
            for cross in (0..cross_dim).step_by(3) {
                let (p1, p2) = match axis {
                    OverlapAxis::Vertical => {
                        let y_a = h_a - cand + c;
                        let y_b = c;
                        (img_a.get_pixel(cross, y_a), img_b.get_pixel(cross, y_b))
                    }
                    OverlapAxis::Horizontal => {
                        let x_a = w_a - cand + c;
                        let x_b = c;
                        (img_a.get_pixel(x_a, cross), img_b.get_pixel(x_b, cross))
                    }
                };

                let l1 = (u32::from(p1[0]) + u32::from(p1[1]) + u32::from(p1[2])) / 3;
                let l2 = (u32::from(p2[0]) + u32::from(p2[1]) + u32::from(p2[2])) / 3;
                sum1 += u64::from(l1);
                sum2 += u64::from(l2);
                count += 1;
            }
        }

        if count == 0 {
            continue;
        }

        let mean1 = sum1 as f64 / count as f64;
        let mean2 = sum2 as f64 / count as f64;

        let mut diff_sum = 0.0f64;
        for c in (0..cand).step_by(step_c as usize) {
            for cross in (0..cross_dim).step_by(3) {
                let (p1, p2) = match axis {
                    OverlapAxis::Vertical => {
                        let y_a = h_a - cand + c;
                        let y_b = c;
                        (img_a.get_pixel(cross, y_a), img_b.get_pixel(cross, y_b))
                    }
                    OverlapAxis::Horizontal => {
                        let x_a = w_a - cand + c;
                        let x_b = c;
                        (img_a.get_pixel(x_a, cross), img_b.get_pixel(x_b, cross))
                    }
                };

                let l1 = (u32::from(p1[0]) + u32::from(p1[1]) + u32::from(p1[2])) as f64 / 3.0;
                let l2 = (u32::from(p2[0]) + u32::from(p2[1]) + u32::from(p2[2])) as f64 / 3.0;

                let zm1 = l1 - mean1;
                let zm2 = l2 - mean2;
                diff_sum += (zm1 - zm2).abs();
            }
        }

        let avg_diff = diff_sum / count as f64;
        if avg_diff < min_avg_diff {
            min_avg_diff = avg_diff;
            best_overlap = cand;
        }
    }

    (best_overlap, min_avg_diff)
}

/// Measures small orthogonal drift between overlapping slices via ECC translation refinement.
#[must_use]
pub fn estimate_orthogonal_drift_ecc(
    img_a: &RgbaImage,
    img_b: &RgbaImage,
    axis: OverlapAxis,
    overlap_px: u32,
) -> i64 {
    if overlap_px < 4 {
        return 0;
    }

    let (w_a, h_a) = img_a.dimensions();
    let (w_b, h_b) = img_b.dimensions();

    let (strip_w, strip_h, max_allowed_drift) = match axis {
        OverlapAxis::Vertical => {
            let width = w_a.min(w_b);
            (width, overlap_px, (width as i64 / 4))
        }
        OverlapAxis::Horizontal => {
            let height = h_a.min(h_b);
            (overlap_px, height, (height as i64 / 4))
        }
    };

    if strip_w == 0 || strip_h == 0 {
        return 0;
    }

    let mut strip_a = Vec::with_capacity((strip_w * strip_h) as usize);
    let mut strip_b = Vec::with_capacity((strip_w * strip_h) as usize);

    match axis {
        OverlapAxis::Vertical => {
            let start_y = h_a.saturating_sub(overlap_px);
            for y in 0..overlap_px {
                for x in 0..strip_w {
                    let p1 = img_a.get_pixel(x, start_y + y);
                    let l1 = (u32::from(p1[0]) + u32::from(p1[1]) + u32::from(p1[2])) / 3;
                    strip_a.push(l1 as u8);

                    let p2 = img_b.get_pixel(x, y);
                    let l2 = (u32::from(p2[0]) + u32::from(p2[1]) + u32::from(p2[2])) / 3;
                    strip_b.push(l2 as u8);
                }
            }
        }
        OverlapAxis::Horizontal => {
            let start_x = w_a.saturating_sub(overlap_px);
            for y in 0..strip_h {
                for x in 0..overlap_px {
                    let p1 = img_a.get_pixel(start_x + x, y);
                    let l1 = (u32::from(p1[0]) + u32::from(p1[1]) + u32::from(p1[2])) / 3;
                    strip_a.push(l1 as u8);

                    let p2 = img_b.get_pixel(x, y);
                    let l2 = (u32::from(p2[0]) + u32::from(p2[1]) + u32::from(p2[2])) / 3;
                    strip_b.push(l2 as u8);
                }
            }
        }
    }

    if let Ok((matrix, _quality)) = register_ecc(
        &strip_a,
        &strip_b,
        strip_w,
        strip_h,
        TransformationType::Translation,
        15,
        1e-3,
    ) {
        let drift = match axis {
            OverlapAxis::Vertical => matrix.data[2].round() as i64,
            OverlapAxis::Horizontal => matrix.data[5].round() as i64,
        };

        if drift.abs() < max_allowed_drift {
            return drift;
        }
    }

    0
}
