use docula_preprocess::{
    AdaptiveBinarizeFilter, HoughDeskewFilter, ImageFilter, OxiMediaClaheFilter,
    OxiMediaDenoiseFilter, OxiMediaFilter, OxiMediaLensUndistortFilter, OxiMediaMotionDeblurFilter,
    OxiMediaPerspectiveDewarpFilter, OxiMediaStitcher, PreprocessedImage, PreprocessingFilterChain,
    ResizeMode, SimdResizeFilter,
};
use image::{Rgba, RgbaImage};
use oximedia_cv::transform::{AffineTransform, warp_affine_image};

/// Helper to generate a test image with clean horizontal dark lines on white background.
fn create_horizontal_lines_image(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));
    let black = Rgba([0, 0, 0, 255]);

    // Draw multiple horizontal ruling lines simulating text baselines
    for y in (50..height.saturating_sub(50)).step_by(40) {
        for x in 40..(width.saturating_sub(40)) {
            img.put_pixel(x, y, black);
            if y + 1 < height {
                img.put_pixel(x, y + 1, black);
            }
        }
    }

    img
}

/// Rotates an image around its center by `angle_deg` degrees with white background fill.
fn rotate_test_image(img: &RgbaImage, angle_deg: f32) -> RgbaImage {
    let (width, height) = img.dimensions();
    let radians = (angle_deg as f64).to_radians();
    let cx = width as f64 / 2.0;
    let cy = height as f64 / 2.0;

    let forward = AffineTransform::rotation_around(radians, cx, cy);
    let inv = forward.inverse().expect("affine rotation is invertible");
    let [[a, b, c], [d, e, f], _] = inv.as_matrix();

    let mut warped = warp_affine_image(
        img.as_raw(),
        width as usize,
        height as usize,
        4,
        [[a, b, c], [d, e, f]],
        width as usize,
        height as usize,
    )
    .expect("warp should succeed");

    for chunk in warped.as_chunks_mut::<4>().0 {
        if chunk[3] == 0 {
            chunk[0] = 255;
            chunk[1] = 255;
            chunk[2] = 255;
            chunk[3] = 255;
        }
    }

    RgbaImage::from_raw(width, height, warped).expect("valid RgbaImage buffer")
}

/// Helper to generate a degraded document with salt-and-pepper scanner noise.
fn create_noisy_document_image(width: u32, height: u32) -> RgbaImage {
    let mut img = create_horizontal_lines_image(width, height);

    // Inject isolated dark pepper noise speck into bright background away from lines (y=20)
    img.put_pixel(50, 20, Rgba([15, 15, 15, 255]));

    // Inject isolated white salt noise speck inside dark text lines (y=50)
    img.put_pixel(100, 50, Rgba([250, 250, 250, 255]));

    img
}

/// Helper to generate an out-of-focus blurred document image.
fn create_blurred_document_image(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));

    // Draw lines with soft blurred edges (transition: 200 -> 120 -> 40 -> 120 -> 200)
    for y_center in (60..height.saturating_sub(60)).step_by(50) {
        for x in 40..(width.saturating_sub(40)) {
            img.put_pixel(x, y_center.saturating_sub(2), Rgba([210, 210, 210, 255]));
            img.put_pixel(x, y_center.saturating_sub(1), Rgba([130, 130, 130, 255]));
            img.put_pixel(x, y_center, Rgba([45, 45, 45, 255]));
            if y_center + 1 < height {
                img.put_pixel(x, y_center + 1, Rgba([130, 130, 130, 255]));
            }
            if y_center + 2 < height {
                img.put_pixel(x, y_center + 2, Rgba([210, 210, 210, 255]));
            }
        }
    }

    img
}

/// Helper to generate a low-contrast faded carbon-copy document image.
fn create_faded_document_image(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([245, 245, 245, 255]));

    // Draw very faint text lines (luma ~185 against background ~245)
    for y in (50..height.saturating_sub(50)).step_by(40) {
        for x in 40..(width.saturating_sub(40)) {
            img.put_pixel(x, y, Rgba([185, 185, 185, 255]));
            if y + 1 < height {
                img.put_pixel(x, y + 1, Rgba([190, 190, 190, 255]));
            }
        }
    }

    img
}

#[test]
fn test_simd_resize_aspect_ratio() {
    // 1920x1080 scaled to width 960 must yield height 540
    let img = RgbaImage::new(1920, 1080);
    let mut target = img.clone();

    let resizer = SimdResizeFilter::fit_width(960);
    let (tw, th) = resizer.compute_target_dimensions(1920, 1080).unwrap();
    assert_eq!((tw, th), (960, 540));

    resizer.apply(&mut target).expect("resize to fit_width");
    assert_eq!(target.dimensions(), (960, 540));
}

#[test]
fn test_simd_resize_modes() {
    let mut img = RgbaImage::new(400, 200);

    // Exact
    let exact = SimdResizeFilter::exact(100, 50);
    let mut exact_img = img.clone();
    exact.apply(&mut exact_img).expect("exact resize");
    assert_eq!(exact_img.dimensions(), (100, 50));

    // Fit bounding box
    let fit = SimdResizeFilter::fit(100, 100);
    let mut fit_img = img.clone();
    fit.apply(&mut fit_img).expect("fit resize");
    // 400x200 fitted into 100x100 -> ratio is 100/400 = 0.25 -> 100x50
    assert_eq!(fit_img.dimensions(), (100, 50));

    // Fit height
    let fit_h = SimdResizeFilter::fit_height(100);
    let mut fit_h_img = img.clone();
    fit_h.apply(&mut fit_h_img).expect("fit height");
    assert_eq!(fit_h_img.dimensions(), (200, 100));

    // Scale
    let scale = SimdResizeFilter::new(ResizeMode::Scale {
        factor_x: 0.5,
        factor_y: 0.5,
    });
    scale.apply(&mut img).expect("scale");
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
    let rotated_15 = rotate_test_image(&base_img, 15.0);

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
    deskewer.apply(&mut deskewed_img).expect("deskew apply");

    // Re-detect skew on the corrected image: should be near 0°
    if let Some(residual_skew) = deskewer
        .detect_skew_angle(&deskewed_img)
        .expect("residual detection")
    {
        assert!(
            residual_skew.abs() <= 1.5,
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
fn test_deskew_below_min_threshold_ignored() {
    let base_img = create_horizontal_lines_image(200, 200);
    let mut test_img = base_img.clone();

    // Default min_threshold is 0.1°. With zero skew, apply() is a no-op and preserves image
    let deskewer = HoughDeskewFilter::new();
    deskewer.apply(&mut test_img).expect("deskew apply");

    assert_eq!(test_img.dimensions(), base_img.dimensions());
}

#[test]
fn test_deskew_empty_image_error() {
    let img = RgbaImage::new(0, 0);
    let deskewer = HoughDeskewFilter::new();
    let err = deskewer.detect_skew_angle(&img).unwrap_err();
    assert!(err.to_string().contains("empty"));
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
fn test_adaptive_binarize_zero_radius_rejected() {
    let mut img = create_horizontal_lines_image(100, 100);
    let binarizer = AdaptiveBinarizeFilter::adaptive(0);

    let err = binarizer.apply(&mut img).unwrap_err();
    assert!(err.to_string().contains("block_radius must be positive"));
}

#[test]
fn test_adaptive_binarize_empty_image_error() {
    let mut img = RgbaImage::new(0, 0);
    let binarizer = AdaptiveBinarizeFilter::default();

    let err = binarizer.apply(&mut img).unwrap_err();
    assert!(err.to_string().contains("empty"));
}

#[test]
fn test_filter_chain_composition() {
    let base_img = create_horizontal_lines_image(800, 600);

    // Rotate by 10 degrees
    let rotated = rotate_test_image(&base_img, 10.0);

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

#[test]
fn test_degraded_document_salt_pepper_noise_handling() {
    let noisy_img = create_noisy_document_image(300, 300);

    // Verify initial degraded condition
    assert_eq!(noisy_img.get_pixel(50, 20), &Rgba([15, 15, 15, 255]));

    let denoise_filter = OxiMediaDenoiseFilter::salt_and_pepper(1);
    let mut cleaned_img = noisy_img;
    denoise_filter
        .apply(&mut cleaned_img)
        .expect("denoise filter must succeed");

    // The dark pepper noise on background must be normalized
    let bg_pixel = cleaned_img.get_pixel(50, 20);
    assert!(
        bg_pixel[0] > 180,
        "Dark pepper noise was not normalized: pixel={:?}",
        bg_pixel
    );
}

#[test]
fn test_degraded_document_blur_contrast_enhancement() {
    let blurred_img = create_blurred_document_image(300, 300);

    let mut enhanced_img = blurred_img;

    // Use unsharp mask sharpening filter
    let sharpen_filter = OxiMediaFilter::new().with_unsharp_mask(2.0, 0.7, 0.0);
    sharpen_filter
        .apply(&mut enhanced_img)
        .expect("sharpen filter must succeed");

    assert_eq!(enhanced_img.dimensions(), (300, 300));

    // Follow with Otsu binarization to verify crisp separation
    let binarize_filter = AdaptiveBinarizeFilter::otsu();
    binarize_filter
        .apply(&mut enhanced_img)
        .expect("binarization must succeed on enhanced blurred image");

    let stroke_pixel = enhanced_img.get_pixel(50, 60);
    assert_eq!(
        stroke_pixel,
        &Rgba([0, 0, 0, 255]),
        "Text stroke should be crisp black"
    );
    let bg_pixel = enhanced_img.get_pixel(50, 20);
    assert_eq!(
        bg_pixel,
        &Rgba([255, 255, 255, 255]),
        "Background should be crisp white"
    );
}

#[test]
fn test_degraded_document_faded_low_contrast() {
    let faded_img = create_faded_document_image(300, 300);

    // Initial contrast is faint: background=245, text=185
    let p_text_init = faded_img.get_pixel(60, 50);
    assert!(p_text_init[0] > 170 && p_text_init[0] < 200);

    // Adaptive thresholding resolves low-contrast local gradients
    let mut binarized_img = faded_img;
    let adaptive_filter = AdaptiveBinarizeFilter::adaptive(12);
    adaptive_filter
        .apply(&mut binarized_img)
        .expect("adaptive filter must succeed on low-contrast input");

    // The faint text line must be binarized to black
    let p_text_bin = binarized_img.get_pixel(60, 50);
    assert_eq!(
        p_text_bin,
        &Rgba([0, 0, 0, 255]),
        "Low-contrast faint ink must resolve to black"
    );

    // The surrounding background must remain white
    let p_bg_bin = binarized_img.get_pixel(60, 20);
    assert_eq!(
        p_bg_bin,
        &Rgba([255, 255, 255, 255]),
        "Background must remain pure white"
    );
}

#[test]
fn test_robust_degraded_document_pipeline_end_to_end() {
    // Generate realistic bad-condition document:
    // 1. Horizontal lines
    // 2. Tilted by 8 degrees
    // 3. Salt and pepper noise
    // 4. Gradient shadow
    let base_img = create_horizontal_lines_image(600, 500);

    let tilted = rotate_test_image(&base_img, 8.0);
    let mut degraded_img = tilted;

    // Inject scanner noise and shadow gradient
    for y in 0..500 {
        for x in 0..600 {
            let p = degraded_img.get_pixel_mut(x, y);
            // Shadow gradient from top to bottom
            let shadow = (y as f32 / 500.0 * 60.0) as u8;
            p[0] = p[0].saturating_sub(shadow);
            p[1] = p[1].saturating_sub(shadow);
            p[2] = p[2].saturating_sub(shadow);

            // Sporadic salt and pepper noise
            if (x * 37 + y * 53) % 293 == 0 {
                p[0] = 10;
                p[1] = 10;
                p[2] = 10;
            } else if (x * 47 + y * 71) % 439 == 0 {
                p[0] = 250;
                p[1] = 250;
                p[2] = 250;
            }
        }
    }

    // Execute the robust degraded document pipeline
    let pipeline = PreprocessingFilterChain::robust_degraded_document_pipeline();
    assert_eq!(pipeline.len(), 4);

    let preprocessed = pipeline
        .process_to_preprocessed(degraded_img)
        .expect("robust pipeline must succeed on hostile degraded document");

    assert_eq!(preprocessed.processed_buffer.width(), 600);
    assert_eq!(preprocessed.processed_buffer.height(), 500);

    // Grayscale buffer output must have valid variance and be fully binarized
    let gray = preprocessed.gray_buffer;
    let black_count = gray.pixels().filter(|p| p[0] == 0).count();
    let white_count = gray.pixels().filter(|p| p[0] == 255).count();
    assert_eq!(
        black_count + white_count,
        (600 * 500) as usize,
        "All pixels must be binary black or white"
    );
    assert!(black_count > 500, "Text lines must be captured");
    assert!(white_count > black_count, "Background must be predominant");
}

#[test]
fn test_oximedia_clahe_and_illumination_filter() {
    let mut img = RgbaImage::from_pixel(100, 100, Rgba([100, 100, 100, 255]));
    // Add artificial shadow gradient
    for y in 0..100 {
        for x in 0..100 {
            let shadow = ((x + y) / 2) as u8;
            img.put_pixel(
                x,
                y,
                Rgba([
                    180u8.saturating_sub(shadow),
                    180u8.saturating_sub(shadow),
                    180u8.saturating_sub(shadow),
                    255,
                ]),
            );
        }
    }

    let filter = OxiMediaClaheFilter::new()
        .with_tiles(4, 4)
        .with_clip_limit(2.0);
    assert_eq!(filter.name(), "OxiMediaClaheFilter");
    filter.apply(&mut img).expect("clahe filter must succeed");
    assert_eq!(img.dimensions(), (100, 100));
}

#[test]
fn test_oximedia_lens_undistort_filter() {
    let mut img = RgbaImage::from_pixel(64, 64, Rgba([200, 200, 200, 255]));
    let filter = OxiMediaLensUndistortFilter::new(60.0, 0.15);
    assert_eq!(filter.name(), "OxiMediaLensUndistortFilter");
    filter.apply(&mut img).expect("lens undistort must succeed");
    assert_eq!(img.dimensions(), (64, 64));
}

#[test]
fn test_oximedia_perspective_dewarp_filter() {
    let mut img = RgbaImage::from_pixel(100, 100, Rgba([30, 30, 30, 255]));
    // Draw a bright document sheet in center
    for y in 20..80 {
        for x in 20..80 {
            img.put_pixel(x, y, Rgba([240, 240, 240, 255]));
        }
    }

    let filter = OxiMediaPerspectiveDewarpFilter::new(0.15);
    assert_eq!(filter.name(), "OxiMediaPerspectiveDewarpFilter");
    filter
        .apply(&mut img)
        .expect("perspective dewarp must succeed");
    assert_eq!(img.dimensions(), (100, 100));
}

#[test]
fn test_oximedia_motion_deblur_filter() {
    let mut img = RgbaImage::from_pixel(32, 32, Rgba([150, 150, 150, 255]));
    let filter = OxiMediaMotionDeblurFilter::default();
    assert_eq!(filter.name(), "OxiMediaMotionDeblurFilter");
    filter.apply(&mut img).expect("motion deblur must succeed");
    assert_eq!(img.dimensions(), (32, 32));
}

#[test]
fn test_oximedia_stitch_horizontal_and_feathered() {
    let left = RgbaImage::from_pixel(50, 40, Rgba([120, 120, 120, 255]));
    let right = RgbaImage::from_pixel(50, 40, Rgba([220, 220, 220, 255]));

    let stitched = OxiMediaStitcher::stitch_horizontal(&left, &right, 10)
        .expect("horizontal stitch must succeed");
    assert_eq!(stitched.dimensions(), (90, 40));

    // Test feathered multi-frame stitcher
    let frames = [(&left, 0i64, 0i64), (&right, 40i64, 0i64)];
    let feathered = OxiMediaStitcher::stitch_frames_feathered(&frames, 10)
        .expect("feathered panorama stitch must succeed");
    assert_eq!(feathered.dimensions(), (90, 40));
}

#[test]
fn test_bad_shape_reconstruction_pipeline_execution() {
    let mut img = RgbaImage::from_pixel(120, 120, Rgba([160, 160, 160, 255]));
    // Simulate folded paper crease
    for y in 55..65 {
        for x in 0..120 {
            img.put_pixel(x, y, Rgba([80, 80, 80, 255]));
        }
    }

    let pipeline = PreprocessingFilterChain::bad_shape_reconstruction_pipeline();
    assert_eq!(pipeline.len(), 4);

    let result = pipeline
        .process_to_preprocessed(img)
        .expect("bad shape reconstruction pipeline must process degraded image");
    assert_eq!(result.original_dimensions(), (120, 120));
}
