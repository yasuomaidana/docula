//! Perspective homography quadrilateral unwarping and document dewarping.

use crate::error::MosaicError;
use image::RgbaImage;
use oximedia_cv::contour::{ContourApprox, ContourConfig, ContourFinder};
use oximedia_cv::transform::perspective::{quad_to_rect, warp_perspective};

/// Unwarps a quadrilateral region from an RGBA image into an axis-aligned rectangular destination image.
pub fn unwarp_quad_rgba(
    parent: &RgbaImage,
    quad: &[(f64, f64); 4],
    dst_w: u32,
    dst_h: u32,
) -> Result<RgbaImage, MosaicError> {
    let (src_w, src_h) = parent.dimensions();
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return Err(MosaicError::InvalidDimensions {
            width: dst_w,
            height: dst_h,
            reason: "Dimensions must be non-zero for unwarping".to_string(),
        });
    }

    let is_axis_aligned = (quad[0].1 - quad[1].1).abs() < 1.0
        && (quad[3].1 - quad[2].1).abs() < 1.0
        && (quad[0].0 - quad[3].0).abs() < 1.0
        && (quad[1].0 - quad[2].0).abs() < 1.0;

    if is_axis_aligned {
        let x0 = (quad[0].0.max(0.0) as u32).min(src_w.saturating_sub(1));
        let y0 = (quad[0].1.max(0.0) as u32).min(src_h.saturating_sub(1));
        let w = dst_w.min(src_w.saturating_sub(x0));
        let h = dst_h.min(src_h.saturating_sub(y0));

        let mut cropped = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                cropped.put_pixel(x, y, *parent.get_pixel(x0 + x, y0 + y));
            }
        }
        return Ok(cropped);
    }

    let transform = quad_to_rect(quad, f64::from(dst_w), f64::from(dst_h))
        .map_err(|e| MosaicError::PerspectiveError(format!("Homography failed: {e}")))?;

    let mut rectified = RgbaImage::new(dst_w, dst_h);
    for c in 0..4 {
        let chan: Vec<u8> = parent.pixels().map(|p| p[c]).collect();
        let warped_c = warp_perspective(&chan, src_w, src_h, &transform, dst_w, dst_h)
            .map_err(|e| MosaicError::PerspectiveError(format!("Channel {c} warp failed: {e}")))?;

        for y in 0..dst_h {
            for x in 0..dst_w {
                let idx = (y * dst_w + x) as usize;
                rectified.get_pixel_mut(x, y)[c] = warped_c[idx];
            }
        }
    }

    Ok(rectified)
}

/// Automatically detects dominant document boundary quad and dewarps tilted page if found.
pub fn auto_dewarp_dominant_quad(
    img: &mut RgbaImage,
    min_area_fraction: f32,
) -> Result<(), MosaicError> {
    let (width, height) = img.dimensions();
    if width < 10 || height < 10 {
        return Ok(());
    }

    let total_area = (width as f32) * (height as f32);
    let min_area = total_area * min_area_fraction.clamp(0.05, 0.95);

    let mut gray = Vec::with_capacity((width * height) as usize);
    for pixel in img.pixels() {
        let luma =
            (u32::from(pixel[0]) * 299 + u32::from(pixel[1]) * 587 + u32::from(pixel[2]) * 114)
                / 1000;
        gray.push(luma as u8);
    }

    let finder = ContourFinder::with_config(ContourConfig {
        approx: ContourApprox::Simple,
        min_area,
        max_area: 0.0,
        retrieve_holes: false,
    });

    let contours = finder.find_contours(&gray, width as usize, height as usize);
    let candidates = finder.filter_by_area(&contours);

    for contour in candidates {
        if contour.points.len() >= 4
            && let Some((min_x, min_y, max_x, max_y)) = contour.bounding_box()
        {
            let w = (max_x - min_x).max(1) as f64;
            let h = (max_y - min_y).max(1) as f64;

            if w >= (width as f64 * 0.96) && h >= (height as f64 * 0.96) {
                return Ok(());
            }

            let quad = [
                (min_x as f64, min_y as f64),
                (max_x as f64, min_y as f64),
                (max_x as f64, max_y as f64),
                (min_x as f64, max_y as f64),
            ];

            let unwarped = unwarp_quad_rgba(img, &quad, width, height)?;
            *img = unwarped;
            return Ok(());
        }
    }

    Ok(())
}
