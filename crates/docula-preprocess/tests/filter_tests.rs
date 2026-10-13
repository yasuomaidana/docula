use docula_preprocess::{
    AdaptiveBinarizeFilter, HoughDeskewFilter, PreprocessedImage, PreprocessingFilterChain,
    ResizeMode, SimdResizeFilter,
};
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_line_segment_mut;
use imageproc::geometric_transformations::{Interpolation, rotate_about_center};

/// Helper to generate a test image with clean horizontal dark lines on white background.
fn create_horizontal_lines_image(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));
    let black = Rgba([0, 0, 0, 255]);

    // Draw multiple horizontal ruling lines simulating text baselines
    for y in (50..height.saturating_sub(50)).step_by(40) {
        draw_line_segment_mut(
            &mut img,
            (40.0, y as f32),
            ((width - 40) as f32, y as f32),
            black,
        );
        // Thicken the line to 2px
        draw_line_segment_mut(
            &mut img,
            (40.0, (y + 1) as f32),
            ((width - 40) as f32, (y + 1) as f32),
            black,
        );
    }

    img
}

#[test]
fn test_simd_resize_aspect_ratio() {
    // 1920x1080 scaled to width 960 must yield height 540
    let img = RgbaImage::new(1920, 1080);
    let mut target = img.clone();

    let resizer = SimdResizeFilter::fit_width(960);
    resizer
        .compute_target_dimensions(1920, 1080)
        .expect("valid target dimensions");

    let (w, h) = resizer
        .compute_target_dimensions(1920, 1080)
        .expect("target computation");
    assert_eq!(w, 960);
    assert_eq!(h, 540);

    use docula_preprocess::ImageFilter;
    resizer.apply(&mut target).expect("resize succeeded");
    assert_eq!(target.width(), 960);
    assert_eq!(target.height(), 540);
}

#[test]
fn test_simd_resize_modes() {
    let mut img = RgbaImage::new(400, 200);

    // Exact
    let exact = SimdResizeFilter::exact(100, 50);
    let mut exact_img = img.clone();
    docula_preprocess::ImageFilter::apply(&exact, &mut exact_img).expect("exact resize");
    assert_eq!(exact_img.dimensions(), (100, 50));

    // Fit bounding box
    let fit = SimdResizeFilter::fit(100, 100);
    let mut fit_img = img.clone();
    docula_preprocess::ImageFilter::apply(&fit, &mut fit_img).expect("fit resize");
    // 400x200 fitted into 100x100 -> ratio is 100/400 = 0.25 -> 100x50
    assert_eq!(fit_img.dimensions(), (100, 50));

    // Fit height
    let fit_h = SimdResizeFilter::fit_height(100);
    let mut fit_h_img = img.clone();
    docula_preprocess::ImageFilter::apply(&fit_h, &mut fit_h_img).expect("fit height");
    assert_eq!(fit_h_img.dimensions(), (200, 100));

    // Scale
    let scale = SimdResizeFilter::new(ResizeMode::Scale {
        factor_x: 0.5,
        factor_y: 0.5,
    });
    docula_preprocess::ImageFilter::apply(&scale, &mut img).expect("scale");
    assert_eq!(img.dimensions(), (200, 100));
}

#[test]
fn test_simd_resize_zero_dimension_rejected() {
    let resizer = SimdResizeFilter::exact(0, 100);
    let err = resizer.compute_target_dimensions(400, 200).unwrap_err();
    assert!(err.to_string().contains("positive"));
}

#[test]
fn test_deskew_rotated_image() {
    // Generate horizontal lines image
    let base_img = create_horizontal_lines_image(600, 600);

    // Rotate by 15.0 degrees clockwise
    let rotated_15 = rotate_about_center(
        &base_img,
        15.0f32.to_radians(),
        Interpolation::Bilinear,
        Rgba([255, 255, 255, 255]),
    );

    let deskewer = HoughDeskewFilter::new().with_vote_threshold(80);

    // Detect skew angle: must detect 15.0° ± 0.5°
    let detected_skew = deskewer
        .detect_skew_angle(&rotated_15)
        .expect("detection should succeed")
        .expect("should identify dominant skew");

    assert!(
        (detected_skew - 15.0).abs() <= 0.5,
        "Detected skew {detected_skew}° is not within ±0.5° of expected 15.0°"
    );

    // Apply deskew filter
    let mut deskewed_img = rotated_15;
    use docula_preprocess::ImageFilter;
    deskewer.apply(&mut deskewed_img).expect("deskew apply");

    // Re-detect skew on the corrected image: should be near 0°
    if let Some(residual_skew) = deskewer
        .detect_skew_angle(&deskewed_img)
        .expect("residual detection")
    {
        assert!(
            residual_skew.abs() <= 0.6,
            "Residual skew {residual_skew}° should be near 0°"
        );
    }
}

#[test]
fn test_deskew_zero_skew_no_op() {
    let base_img = create_horizontal_lines_image(500, 500);
    let deskewer = HoughDeskewFilter::new().with_vote_threshold(80);

    let angle = deskewer.detect_skew_angle(&base_img).expect("detection");
    if let Some(detected) = angle {
        assert!(
            detected.abs() <= 0.5,
            "Horizontal lines should have near 0 skew, got {detected}"
        );
    }
}

#[test]
fn test_adaptive_binarize_gradient_shadow() {
    let width = 200;
    let height = 200;
    let mut img = RgbaImage::new(width, height);

    // Simulate uneven illumination: brightness fades from 240 on left to 100 on right
    for y in 0..height {
        for x in 0..width {
            let bg_luma = (240.0 - (x as f32 / width as f32) * 140.0) as u8;
            img.put_pixel(x, y, Rgba([bg_luma, bg_luma, bg_luma, 255]));
        }
    }

    // Add dark text strokes across both bright and dark regions
    for y in [40, 80, 120, 160] {
        for x in 20..180 {
            let p = img.get_pixel_mut(x, y);
            // Dark stroke is significantly darker than surrounding background
            p[0] = p[0].saturating_sub(70);
            p[1] = p[1].saturating_sub(70);
            p[2] = p[2].saturating_sub(70);
        }
    }

    let binarizer = AdaptiveBinarizeFilter::adaptive(10);
    use docula_preprocess::ImageFilter;
    binarizer.apply(&mut img).expect("binarization succeeded");

    // Assert that every pixel is strictly black (0) or white (255)
    for pixel in img.pixels() {
        assert!(
            (pixel[0] == 0 || pixel[0] == 255) && pixel[0] == pixel[1] && pixel[1] == pixel[2],
            "Pixel {:?} is not strictly binary",
            pixel
        );
    }

    // Assert that text strokes were extracted as black (0) in both bright and dark zones
    assert_eq!(img.get_pixel(50, 40)[0], 0, "bright region stroke is black");
    assert_eq!(img.get_pixel(150, 40)[0], 0, "dark region stroke is black");
}

#[test]
fn test_adaptive_binarize_otsu() {
    let mut img = create_horizontal_lines_image(100, 100);
    let otsu_binarizer = AdaptiveBinarizeFilter::otsu();

    use docula_preprocess::ImageFilter;
    otsu_binarizer.apply(&mut img).expect("otsu binarize");

    for pixel in img.pixels() {
        assert!(
            pixel[0] == 0 || pixel[0] == 255,
            "Pixel {:?} must be strictly binary",
            pixel
        );
    }
}

#[test]
fn test_filter_chain_composition() {
    let base_img = create_horizontal_lines_image(800, 600);

    // Rotate by 10 degrees
    let rotated = rotate_about_center(
        &base_img,
        10.0f32.to_radians(),
        Interpolation::Bilinear,
        Rgba([255, 255, 255, 255]),
    );

    let chain = PreprocessingFilterChain::new()
        .with_filter(SimdResizeFilter::fit_width(400))
        .with_filter(HoughDeskewFilter::new().with_vote_threshold(50))
        .with_filter(AdaptiveBinarizeFilter::adaptive(8));

    assert_eq!(chain.len(), 3);
    assert!(!chain.is_empty());

    let preprocessed = chain
        .process_to_preprocessed(rotated)
        .expect("processing succeeded");

    assert_eq!(preprocessed.original_width, 800);
    assert_eq!(preprocessed.original_height, 600);
    assert_eq!(preprocessed.processed_buffer.width(), 400);
    assert_eq!(preprocessed.processed_buffer.height(), 300);
    assert_eq!(preprocessed.scale_factor_x, 0.5);
    assert_eq!(preprocessed.scale_factor_y, 0.5);

    // Verify tensor
    assert_eq!(preprocessed.tensor_shape(), (1, 3, 300, 400));
    assert_eq!(preprocessed.rgb_normalized_tensor.len(), 3 * 400 * 300);

    // Tensor values must be clamped in [0.0, 1.0]
    for &val in &preprocessed.rgb_normalized_tensor {
        assert!(
            (0.0..=1.0).contains(&val),
            "Tensor value {val} out of bounds"
        );
    }
}

#[test]
fn test_preprocessed_image_dimensions() {
    let img = RgbaImage::new(100, 50);
    let prep = PreprocessedImage::from_rgba(200, 100, img);
    assert_eq!(prep.original_dimensions(), (200, 100));
    assert_eq!(prep.processed_dimensions(), (100, 50));
    assert_eq!(prep.scale_factor_x, 0.5);
    assert_eq!(prep.scale_factor_y, 0.5);
}

#[test]
fn test_oximedia_filter_and_stitcher() {
    use docula_preprocess::{ImageFilter, OxiMediaFilter, OxiMediaStitcher};

    let mut img = RgbaImage::from_pixel(80, 80, Rgba([128, 128, 128, 255]));
    img.put_pixel(10, 10, Rgba([200, 200, 200, 255]));

    let filter = OxiMediaFilter::new().with_contrast(1.5);
    assert_eq!(filter.name(), "OxiMediaCvFilter");
    filter.apply(&mut img).expect("oximedia filter applied");

    // Stitching two images
    let top = RgbaImage::from_pixel(50, 40, Rgba([100, 100, 100, 255]));
    let bottom = RgbaImage::from_pixel(50, 40, Rgba([200, 200, 200, 255]));
    let stitched = OxiMediaStitcher::stitch_vertical(&top, &bottom, 10).expect("stitched");
    assert_eq!(stitched.dimensions(), (50, 70));
}
