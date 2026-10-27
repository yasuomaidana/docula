//! Image stitching and multi-frame feathered composition.

use crate::error::MosaicError;
use image::{Rgba, RgbaImage};
use oximedia_cv::panorama_stitch::{
    PanoFormat, PanoramaConfig, PanoramaStitcher, StitchFrame,
};

/// Merges two vertically overlapping document image sections into a unified canvas.
pub fn stitch_pair_vertical(
    top: &RgbaImage,
    bottom: &RgbaImage,
    overlap_height: u32,
) -> Result<RgbaImage, MosaicError> {
    let (w_top, h_top) = top.dimensions();
    let (w_bottom, h_bottom) = bottom.dimensions();

    if w_top == 0 || h_top == 0 || w_bottom == 0 || h_bottom == 0 {
        return Err(MosaicError::InvalidDimensions {
            width: w_top,
            height: h_top,
            reason: "Dimensions must be non-zero for vertical stitching".to_string(),
        });
    }

    let unified_width = w_top.max(w_bottom);
    let actual_overlap = overlap_height.min(h_top).min(h_bottom);
    let unified_height = h_top + h_bottom - actual_overlap;

    let mut canvas =
        RgbaImage::from_pixel(unified_width, unified_height, Rgba([255, 255, 255, 255]));

    for y in 0..h_top {
        for x in 0..w_top {
            canvas.put_pixel(x, y, *top.get_pixel(x, y));
        }
    }

    let seam_start = h_top - actual_overlap;
    for y in 0..h_bottom {
        let canvas_y = seam_start + y;
        for x in 0..w_bottom {
            let bottom_pixel = bottom.get_pixel(x, y);
            if y < actual_overlap {
                let alpha = y as f32 / actual_overlap as f32;
                let top_pixel = canvas.get_pixel(x, canvas_y);
                let blended_r =
                    ((1.0 - alpha) * top_pixel[0] as f32 + alpha * bottom_pixel[0] as f32) as u8;
                let blended_g =
                    ((1.0 - alpha) * top_pixel[1] as f32 + alpha * bottom_pixel[1] as f32) as u8;
                let blended_b =
                    ((1.0 - alpha) * top_pixel[2] as f32 + alpha * bottom_pixel[2] as f32) as u8;
                canvas.put_pixel(x, canvas_y, Rgba([blended_r, blended_g, blended_b, 255]));
            } else {
                canvas.put_pixel(x, canvas_y, *bottom_pixel);
            }
        }
    }

    Ok(canvas)
}

/// Merges two horizontally overlapping document image sections into a unified canvas.
pub fn stitch_pair_horizontal(
    left: &RgbaImage,
    right: &RgbaImage,
    overlap_width: u32,
) -> Result<RgbaImage, MosaicError> {
    let (w_left, h_left) = left.dimensions();
    let (w_right, h_right) = right.dimensions();

    if w_left == 0 || h_left == 0 || w_right == 0 || h_right == 0 {
        return Err(MosaicError::InvalidDimensions {
            width: w_left,
            height: h_left,
            reason: "Dimensions must be non-zero for horizontal stitching".to_string(),
        });
    }

    let unified_height = h_left.max(h_right);
    let actual_overlap = overlap_width.min(w_left).min(w_right);
    let unified_width = w_left + w_right - actual_overlap;

    let mut canvas =
        RgbaImage::from_pixel(unified_width, unified_height, Rgba([255, 255, 255, 255]));

    for y in 0..h_left {
        for x in 0..w_left {
            canvas.put_pixel(x, y, *left.get_pixel(x, y));
        }
    }

    let seam_start = w_left - actual_overlap;
    for y in 0..h_right {
        for x in 0..w_right {
            let canvas_x = seam_start + x;
            let right_pixel = right.get_pixel(x, y);
            if x < actual_overlap {
                let alpha = x as f32 / actual_overlap as f32;
                let left_pixel = canvas.get_pixel(canvas_x, y);
                let blended_r =
                    ((1.0 - alpha) * left_pixel[0] as f32 + alpha * right_pixel[0] as f32) as u8;
                let blended_g =
                    ((1.0 - alpha) * left_pixel[1] as f32 + alpha * right_pixel[1] as f32) as u8;
                let blended_b =
                    ((1.0 - alpha) * left_pixel[2] as f32 + alpha * right_pixel[2] as f32) as u8;
                canvas.put_pixel(canvas_x, y, Rgba([blended_r, blended_g, blended_b, 255]));
            } else {
                canvas.put_pixel(canvas_x, y, *right_pixel);
            }
        }
    }

    Ok(canvas)
}

/// Stitches multiple RGBA frames with pixel offsets and linear alpha feathering.
pub fn stitch_frames_feathered(
    frames: &[(&RgbaImage, i64, i64)],
    blend_width_px: u32,
) -> Result<RgbaImage, MosaicError> {
    if frames.is_empty() {
        return Err(MosaicError::InvalidDimensions {
            width: 0,
            height: 0,
            reason: "No frames provided for feathered stitching".to_string(),
        });
    }

    let stitch_frames: Vec<StitchFrame> = frames
        .iter()
        .map(|(img, ox, oy)| {
            let (w, h) = img.dimensions();
            StitchFrame {
                image: img.as_raw().clone(),
                width: w,
                height: h,
                offset_x: *ox,
                offset_y: *oy,
            }
        })
        .collect();

    let config = PanoramaConfig {
        blend_width_px,
        output_format: PanoFormat::Flat,
    };

    let stitcher = PanoramaStitcher::new(config);
    let pano = stitcher
        .stitch(&stitch_frames)
        .map_err(|e| MosaicError::StitchError(format!("Panorama stitch failed: {e}")))?;

    RgbaImage::from_raw(pano.width, pano.height, pano.data).ok_or_else(|| {
        MosaicError::StitchError("Failed to construct RgbaImage from panorama buffer".to_string())
    })
}
