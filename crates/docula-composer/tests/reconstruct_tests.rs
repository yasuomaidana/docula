use docula_composer::{ComposerError, DocumentReconstructor, ReconstructionConfig, StitchMode};
use docula_core::DocumentSource;
use docula_preprocess::PageRasterBuffer;
use image::{ImageFormat, Rgba, RgbaImage};

/// Generates a test RGBA pattern with distinct non-periodic vertical gradients and text-like blocks.
fn make_test_pattern(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));

    // Draw unique text blocks and content per line to simulate realistic non-repeating document text
    for y in 0..height {
        for x in 0..width {
            if (x > 10 && x < width - 10) && (y % 16 < 6) {
                let val = (((x * 13 + y * 29) % 180) + 30) as u8;
                img.put_pixel(x, y, Rgba([val, val, val, 255]));
            }
        }
    }

    img
}

#[test]
fn test_reconstruct_empty_fragments_error() {
    let reconstructor = DocumentReconstructor::new(ReconstructionConfig::default());
    let err = reconstructor.reconstruct_from_buffers(&[]).unwrap_err();
    assert!(matches!(err, ComposerError::EmptyFragments));

    let err2 = reconstructor.reconstruct_from_sources(&[]).unwrap_err();
    assert!(matches!(err2, ComposerError::EmptyFragments));
}

#[test]
fn test_reconstruct_single_fragment_pass_through() {
    let reconstructor = DocumentReconstructor::new(ReconstructionConfig::default());
    let img = make_test_pattern(100, 150);
    let buffer = PageRasterBuffer::new(0, img, 300).unwrap();

    let reconstructed = reconstructor
        .reconstruct_from_buffers(&[buffer])
        .expect("Single buffer reconstruction must succeed");

    assert_eq!(reconstructed.stitched_width, 100);
    assert_eq!(reconstructed.stitched_height, 150);
    assert_eq!(reconstructed.fragment_count, 1);
    assert_eq!(reconstructed.estimated_coverage_ratio, 1.0);
    assert!(reconstructed.alignments.is_empty());
}

#[test]
fn test_reconstruct_vertical_two_overlapping_slices() {
    let full = make_test_pattern(120, 200);

    // Slice into top [0..120] and bottom [80..200], overlap is 40 pixels [80..120]
    let mut top_slice = RgbaImage::new(120, 120);
    for y in 0..120 {
        for x in 0..120 {
            top_slice.put_pixel(x, y, *full.get_pixel(x, y));
        }
    }

    let mut bot_slice = RgbaImage::new(120, 120);
    for y in 0..120 {
        for x in 0..120 {
            bot_slice.put_pixel(x, y, *full.get_pixel(x, 80 + y));
        }
    }

    let config = ReconstructionConfig {
        stitch_mode: StitchMode::VerticalStrip,
        feather_overlap: true,
        blend_width_px: 16,
        apply_bad_shape_filters: false,
        ..Default::default()
    };

    let reconstructor = DocumentReconstructor::new(config);
    let buf_top = PageRasterBuffer::new(0, top_slice, 300).unwrap();
    let buf_bot = PageRasterBuffer::new(1, bot_slice, 300).unwrap();

    let reconstructed = reconstructor
        .reconstruct_from_buffers(&[buf_top, buf_bot])
        .expect("Vertical reconstruction must succeed");

    assert_eq!(reconstructed.fragment_count, 2);
    assert_eq!(reconstructed.stitched_width, 120);
    // 120 + 120 - overlap(approx 40) => approx 200
    assert!(
        reconstructed.stitched_height >= 180 && reconstructed.stitched_height <= 220,
        "Stitched height should be ~200, got {}",
        reconstructed.stitched_height
    );
    assert_eq!(reconstructed.alignments.len(), 1);

    // Verify conversion to DocumentSource
    let source = reconstructed
        .to_source(ImageFormat::Png)
        .expect("Export to PNG DocumentSource must succeed");
    assert!(source.is_image());
    assert!(source.infer_document_id().starts_with("doc-"));
}

#[test]
fn test_reconstruct_horizontal_two_overlapping_slices() {
    let full = make_test_pattern(200, 80);

    // Slice into left [0..120] and right [80..200], overlap is 40 pixels [80..120]
    let mut left_slice = RgbaImage::new(120, 80);
    for y in 0..80 {
        for x in 0..120 {
            left_slice.put_pixel(x, y, *full.get_pixel(x, y));
        }
    }

    let mut right_slice = RgbaImage::new(120, 80);
    for y in 0..80 {
        for x in 0..120 {
            right_slice.put_pixel(x, y, *full.get_pixel(80 + x, y));
        }
    }

    let config = ReconstructionConfig {
        stitch_mode: StitchMode::HorizontalStrip,
        feather_overlap: true,
        blend_width_px: 12,
        apply_bad_shape_filters: false,
        ..Default::default()
    };

    let reconstructor = DocumentReconstructor::new(config);
    let buf_left = PageRasterBuffer::new(0, left_slice, 300).unwrap();
    let buf_right = PageRasterBuffer::new(1, right_slice, 300).unwrap();

    let reconstructed = reconstructor
        .reconstruct_from_buffers(&[buf_left, buf_right])
        .expect("Horizontal reconstruction must succeed");

    assert_eq!(reconstructed.fragment_count, 2);
    assert_eq!(reconstructed.stitched_height, 80);
    assert!(
        reconstructed.stitched_width >= 180 && reconstructed.stitched_width <= 220,
        "Stitched width should be ~200, got {}",
        reconstructed.stitched_width
    );
    assert_eq!(reconstructed.alignments.len(), 1);
}

#[test]
fn test_reconstruct_bad_shape_filters_execution() {
    // Test with bad_shape_filters = true on a degraded pattern with gradient lighting
    let mut pattern = make_test_pattern(80, 160);

    // Simulate bad shape: non-uniform lighting shadow
    for y in 0..160 {
        let shadow = (y as f32 / 160.0 * 120.0) as u8;
        for x in 0..80 {
            let p = pattern.get_pixel_mut(x, y);
            p[0] = p[0].saturating_sub(shadow);
            p[1] = p[1].saturating_sub(shadow);
            p[2] = p[2].saturating_sub(shadow);
        }
    }

    // Split into 2 slices
    let mut top_slice = RgbaImage::new(80, 100);
    for y in 0..100 {
        for x in 0..80 {
            top_slice.put_pixel(x, y, *pattern.get_pixel(x, y));
        }
    }
    let mut bot_slice = RgbaImage::new(80, 100);
    for y in 0..100 {
        for x in 0..80 {
            bot_slice.put_pixel(x, y, *pattern.get_pixel(x, 60 + y));
        }
    }

    let config = ReconstructionConfig {
        stitch_mode: StitchMode::Auto,
        apply_bad_shape_filters: true,
        feather_overlap: true,
        ..Default::default()
    };

    let reconstructor = DocumentReconstructor::new(config);
    let buf_top = PageRasterBuffer::new(0, top_slice, 300).unwrap();
    let buf_bot = PageRasterBuffer::new(1, bot_slice, 300).unwrap();

    let reconstructed = reconstructor
        .reconstruct_from_buffers(&[buf_top, buf_bot])
        .expect("Degraded document reconstruction must succeed");

    assert!(reconstructed.stitched_height >= 140);
    assert_eq!(reconstructed.fragment_count, 2);
}

#[test]
fn test_reconstruct_from_document_sources() {
    let top = make_test_pattern(80, 80);
    let bot = make_test_pattern(80, 80);

    let mut top_bytes = Vec::new();
    let mut bot_bytes = Vec::new();
    top.write_to(
        &mut std::io::Cursor::new(&mut top_bytes),
        image::ImageFormat::Png,
    )
    .unwrap();
    bot.write_to(
        &mut std::io::Cursor::new(&mut bot_bytes),
        image::ImageFormat::Png,
    )
    .unwrap();

    let src1 = DocumentSource::from_image_bytes(top_bytes, Some("png"));
    let src2 = DocumentSource::from_image_bytes(bot_bytes, Some("png"));

    let reconstructor = DocumentReconstructor::new(ReconstructionConfig {
        apply_bad_shape_filters: false,
        ..Default::default()
    });

    let reconstructed = reconstructor
        .reconstruct_from_sources(&[src1, src2])
        .expect("Reconstruction from sources must succeed");

    assert_eq!(reconstructed.fragment_count, 2);
    assert!(reconstructed.stitched_height >= 100);
}
