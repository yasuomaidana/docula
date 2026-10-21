use image::{GrayImage, Luma, RgbaImage};

/// Normalized preprocessing artifact containing processed pixel buffers and OCR tensors.
///
/// Preserves the original page dimensions to allow mapping bounding boxes and spatial spans
/// back to the canonical coordinate space.
#[derive(Clone, Debug)]
pub struct PreprocessedImage {
    /// Width of the original input before filtering/scaling.
    pub original_width: u32,
    /// Height of the original input before filtering/scaling.
    pub original_height: u32,
    /// Final processed RGBA image buffer.
    pub processed_buffer: RgbaImage,
    /// Grayscale 8bpp representation of the processed buffer.
    pub gray_buffer: GrayImage,
    /// Normalized planar RGB tensor formatted as `[1, 3, H, W]` in `[0.0, 1.0]`.
    pub rgb_normalized_tensor: Vec<f32>,
    /// Scale factor along the horizontal axis (`processed_width / original_width`).
    pub scale_factor_x: f32,
    /// Scale factor along the vertical axis (`processed_height / original_height`).
    pub scale_factor_y: f32,
}

impl PreprocessedImage {
    /// Constructs a `PreprocessedImage` from an RGBA buffer and its original pre-transform dimensions.
    #[must_use]
    pub fn from_rgba(original_width: u32, original_height: u32, processed: RgbaImage) -> Self {
        let (width, height) = processed.dimensions();
        let orig_w_safe = if original_width == 0 {
            1.0
        } else {
            original_width as f32
        };
        let orig_h_safe = if original_height == 0 {
            1.0
        } else {
            original_height as f32
        };

        let scale_factor_x = width as f32 / orig_w_safe;
        let scale_factor_y = height as f32 / orig_h_safe;

        // Convert to grayscale 8bpp buffer
        let mut gray_buffer = GrayImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let pixel = processed.get_pixel(x, y);
                // Standard Rec. 601 luminance
                let luma = (u32::from(pixel[0]) * 299
                    + u32::from(pixel[1]) * 587
                    + u32::from(pixel[2]) * 114)
                    / 1000;
                gray_buffer.put_pixel(x, y, Luma([luma as u8]));
            }
        }

        // Build planar [1, 3, H, W] normalized float tensor in [0.0, 1.0]
        let num_pixels = (width as usize) * (height as usize);
        let mut tensor = vec![0.0f32; 3 * num_pixels];

        let r_offset = 0;
        let g_offset = num_pixels;
        let b_offset = 2 * num_pixels;

        for y in 0..height {
            for x in 0..width {
                let pixel = processed.get_pixel(x, y);
                let idx = (y as usize) * (width as usize) + (x as usize);
                tensor[r_offset + idx] = f32::from(pixel[0]) / 255.0;
                tensor[g_offset + idx] = f32::from(pixel[1]) / 255.0;
                tensor[b_offset + idx] = f32::from(pixel[2]) / 255.0;
            }
        }

        Self {
            original_width,
            original_height,
            processed_buffer: processed,
            gray_buffer,
            rgb_normalized_tensor: tensor,
            scale_factor_x,
            scale_factor_y,
        }
    }

    /// Returns the shape of the normalized tensor as `(batch, channels, height, width)`.
    #[must_use]
    pub fn tensor_shape(&self) -> (usize, usize, usize, usize) {
        (
            1,
            3,
            self.processed_buffer.height() as usize,
            self.processed_buffer.width() as usize,
        )
    }

    /// Returns the dimensions `(width, height)` of the processed image buffer.
    #[must_use]
    pub fn processed_dimensions(&self) -> (u32, u32) {
        self.processed_buffer.dimensions()
    }

    /// Returns the dimensions `(width, height)` of the original input.
    #[must_use]
    pub fn original_dimensions(&self) -> (u32, u32) {
        (self.original_width, self.original_height)
    }
}
