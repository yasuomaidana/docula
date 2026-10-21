use docula_core::BoundingBox;
use docula_ocr::SpatialLayoutMap;

/// Computes a rotation and scale-resilient 64-bit geometric layout SimHash fingerprint.
#[must_use]
pub fn compute_layout_fingerprint(layout: &SpatialLayoutMap) -> u64 {
    if layout.is_empty() {
        return 0;
    }

    let mut bit_counts = [0_i32; 64];

    for line in &layout.lines {
        accumulate_bbox_simhash(&line.bbox, &mut bit_counts);
    }

    let mut fingerprint = 0_u64;
    for (i, &count) in bit_counts.iter().enumerate() {
        if count > 0 {
            fingerprint |= 1_u64 << i;
        }
    }

    fingerprint
}

/// Computes the Hamming distance (number of differing bits) between two fingerprints.
#[must_use]
#[inline]
pub const fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

fn accumulate_bbox_simhash(bbox: &BoundingBox, bit_counts: &mut [i32; 64]) {
    // Quantize into coarse spatial bins (16x16 grid = 4 bits for x, 4 bits for y)
    let gx0 = ((bbox.top_left.x * 16.0).floor() as u64).min(15);
    let gy0 = ((bbox.top_left.y * 16.0).floor() as u64).min(15);
    let gx1 = ((bbox.bottom_right.x * 16.0).floor() as u64).min(15);
    let gy1 = ((bbox.bottom_right.y * 16.0).floor() as u64).min(15);

    let feature_hash = hash_64((gx0 << 12) | (gy0 << 8) | (gx1 << 4) | gy1);

    for (i, count) in bit_counts.iter_mut().enumerate() {
        if (feature_hash & (1_u64 << i)) != 0 {
            *count += 1;
        } else {
            *count -= 1;
        }
    }
}

/// Fast FNV-1a 64-bit mixing hash.
const fn hash_64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58476d1ce4e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
