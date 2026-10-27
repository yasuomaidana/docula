//! Luminance calculation and channel mapping helpers.

use image::RgbaImage;

/// Calculates ITU-R BT.601 perceived luminance for an RGB triple.
#[inline]
#[must_use]
pub fn rgb_to_luma_u8(r: u8, g: u8, b: u8) -> u8 {
    ((u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1000) as u8
}

/// Converts an RGBA image into an in-memory grayscale byte buffer.
#[must_use]
pub fn rgba_to_luma_vec(img: &RgbaImage) -> Vec<u8> {
    let mut gray = Vec::with_capacity((img.width() * img.height()) as usize);
    for p in img.pixels() {
        gray.push(rgb_to_luma_u8(p[0], p[1], p[2]));
    }
    gray
}

/// Maps each RGB color channel (indices 0..3) independently through a transforming closure.
pub fn map_rgb_channels<F>(img: &mut RgbaImage, mut f: F)
where
    F: FnMut(usize, &[u8], usize, usize) -> Vec<u8>,
{
    let (width, height) = img.dimensions();
    let w = width as usize;
    let h = height as usize;

    for c in 0..3 {
        let mut channel_buf = Vec::with_capacity(w * h);
        for y in 0..height {
            for x in 0..width {
                channel_buf.push(img.get_pixel(x, y)[c]);
            }
        }

        let processed = f(c, &channel_buf, w, h);
        if processed.len() == w * h {
            for y in 0..height {
                for x in 0..width {
                    let idx = (y as usize) * w + (x as usize);
                    img.get_pixel_mut(x, y)[c] = processed[idx];
                }
            }
        }
    }
}

/// Maps each RGBA channel (indices 0..4) independently through a transforming closure.
pub fn map_rgba_channels<F>(img: &mut RgbaImage, mut f: F)
where
    F: FnMut(usize, &[u8], usize, usize) -> Vec<u8>,
{
    let (width, height) = img.dimensions();
    let w = width as usize;
    let h = height as usize;

    for c in 0..4 {
        let mut channel_buf = Vec::with_capacity(w * h);
        for y in 0..height {
            for x in 0..width {
                channel_buf.push(img.get_pixel(x, y)[c]);
            }
        }

        let processed = f(c, &channel_buf, w, h);
        if processed.len() == w * h {
            for y in 0..height {
                for x in 0..width {
                    let idx = (y as usize) * w + (x as usize);
                    img.get_pixel_mut(x, y)[c] = processed[idx];
                }
            }
        }
    }
}
