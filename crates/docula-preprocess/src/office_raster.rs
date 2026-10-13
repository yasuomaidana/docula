use crate::buffer::PageRasterBuffer;
use crate::error::PreprocessError;
use docula_core::SpatialSpan;
use image::{Rgba, RgbaImage};

/// Pure-Rust vector rasterizer rendering ofimatic documents and spatial layouts into visual pixel buffers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OfficeRasterizer;

impl OfficeRasterizer {
    /// Creates a new `OfficeRasterizer`.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Renders a sequence of spatial spans onto an RGBA canvas at specified DPI.
    pub fn render_spans(
        &self,
        spans: &[SpatialSpan],
        page_index: usize,
        dpi: u32,
    ) -> Result<PageRasterBuffer, PreprocessError> {
        let width = ((8.5 * dpi as f32).round() as u32).max(600);
        let height = ((11.0 * dpi as f32).round() as u32).max(800);

        let mut img = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));

        for span in spans {
            let rect = span.bbox.to_pixel_rect(width, height);
            let x_end = (rect.x + rect.width).min(width);
            let y_end = (rect.y + rect.height).min(height);

            // Draw cell/span bounding border
            for x in rect.x..x_end {
                img.put_pixel(x, rect.y, Rgba([220, 224, 230, 255]));
                if y_end > 0 {
                    img.put_pixel(x, y_end - 1, Rgba([220, 224, 230, 255]));
                }
            }
            for y in rect.y..y_end {
                img.put_pixel(rect.x, y, Rgba([220, 224, 230, 255]));
                if x_end > 0 {
                    img.put_pixel(x_end - 1, y, Rgba([220, 224, 230, 255]));
                }
            }

            // Draw text stroke marker
            let mid_y = rect.y + (rect.height / 2).max(1);
            let text_len = span.text.len().min(rect.width as usize);
            let text_x_end = (rect.x + (text_len as u32 * 6)).min(x_end);
            if mid_y < height {
                for x in (rect.x + 2)..text_x_end {
                    img.put_pixel(x, mid_y, Rgba([30, 41, 59, 255]));
                }
            }
        }

        PageRasterBuffer::new(page_index, img, dpi)
    }
}
