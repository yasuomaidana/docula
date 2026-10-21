use docula_core::{BoundingBox, CropFormat, RegionCropper};
use docula_preprocess::{PdfRegionCropper, PreprocessError, Rasterizer, TinySkiaRasterizer};
use pdf_oxide::api::Pdf;

fn generate_test_pdf_bytes(content: &str) -> Vec<u8> {
    let mut pdf = Pdf::from_html(content).expect("Failed to create test PDF from HTML");
    pdf.to_bytes().expect("Failed to convert PDF to bytes")
}

fn generate_two_page_pdf_bytes() -> Vec<u8> {
    let p1 = tempfile::NamedTempFile::new().expect("Failed tempfile 1");
    let p2 = tempfile::NamedTempFile::new().expect("Failed tempfile 2");
    let mut pdf1 = Pdf::from_html("<h1>Page 1</h1>").expect("Failed to create PDF 1");
    pdf1.save(p1.path()).expect("Failed to save PDF 1");
    let mut pdf2 = Pdf::from_html("<h1>Page 2</h1>").expect("Failed to create PDF 2");
    pdf2.save(p2.path()).expect("Failed to save PDF 2");
    pdf_oxide::api::merge_pdfs(&[p1.path(), p2.path()]).expect("Failed to merge PDFs")
}

#[test]
fn test_render_single_page_pdf() {
    let pdf_bytes =
        generate_test_pdf_bytes("<h1>Single Page Test</h1><p>Content for rasterization</p>");
    let rasterizer = TinySkiaRasterizer::new();

    let page_count = rasterizer
        .page_count(&pdf_bytes)
        .expect("Failed to get page count");
    assert_eq!(page_count, 1);

    let buffer_150 = rasterizer
        .render_page(&pdf_bytes, 0, 150)
        .expect("Failed to render at 150 DPI");
    assert_eq!(buffer_150.page_index, 0);
    assert_eq!(buffer_150.dpi, 150);
    assert!(buffer_150.width() > 0);
    assert!(buffer_150.height() > 0);
    assert_eq!(
        buffer_150.as_raw().len(),
        (buffer_150.width() * buffer_150.height() * 4) as usize
    );

    let buffer_300 = rasterizer
        .render_page(&pdf_bytes, 0, 300)
        .expect("Failed to render at 300 DPI");
    assert_eq!(buffer_300.page_index, 0);
    assert_eq!(buffer_300.dpi, 300);

    // 300 DPI should be approximately double the dimensions of 150 DPI
    let ratio_w = buffer_300.width() as f32 / buffer_150.width() as f32;
    let ratio_h = buffer_300.height() as f32 / buffer_150.height() as f32;
    assert!(
        (ratio_w - 2.0).abs() < 0.1,
        "Expected width ratio ~2.0, got {ratio_w}"
    );
    assert!(
        (ratio_h - 2.0).abs() < 0.1,
        "Expected height ratio ~2.0, got {ratio_h}"
    );
}

#[test]
fn test_render_invalid_pdf_bytes() {
    let rasterizer = TinySkiaRasterizer::new();
    let garbage = b"THIS IS DEFINITELY NOT A VALID PDF";
    let res = rasterizer.render_page(garbage, 0, 150);
    match res {
        Err(PreprocessError::ParseFailure { .. }) => {}
        other => panic!("Expected ParseFailure, got {other:?}"),
    }
}

#[test]
fn test_render_empty_payload() {
    let rasterizer = TinySkiaRasterizer::new();
    let res = rasterizer.render_page(&[], 0, 150);
    match res {
        Err(PreprocessError::EmptyPayload) => {}
        other => panic!("Expected EmptyPayload, got {other:?}"),
    }

    let count_res = rasterizer.page_count(&[]);
    match count_res {
        Err(PreprocessError::EmptyPayload) => {}
        other => panic!("Expected EmptyPayload, got {other:?}"),
    }
}

#[test]
fn test_render_invalid_dpi_zero() {
    let pdf_bytes = generate_test_pdf_bytes("<h1>Invalid DPI Test</h1>");
    let rasterizer = TinySkiaRasterizer::new();
    let res = rasterizer.render_page(&pdf_bytes, 0, 0);
    match res {
        Err(PreprocessError::InvalidDpi { dpi: 0 }) => {}
        other => panic!("Expected InvalidDpi(0), got {other:?}"),
    }
}

#[test]
fn test_render_page_out_of_bounds() {
    let pdf_bytes = generate_test_pdf_bytes("<h1>Single Page</h1>");
    let rasterizer = TinySkiaRasterizer::new();
    let res = rasterizer.render_page(&pdf_bytes, 99, 150);
    match res {
        Err(PreprocessError::PageOutOfBounds {
            page_index,
            total_pages,
        }) => {
            assert_eq!(page_index, 99);
            assert_eq!(total_pages, 1);
        }
        other => panic!("Expected PageOutOfBounds, got {other:?}"),
    }
}

#[test]
fn test_render_multi_page_pdf() {
    let pdf_bytes = generate_two_page_pdf_bytes();
    let rasterizer = TinySkiaRasterizer::new();

    let count = rasterizer
        .page_count(&pdf_bytes)
        .expect("Failed page count");
    assert_eq!(count, 2);

    let all_buffers = rasterizer
        .render_all_pages(&pdf_bytes, 150)
        .expect("Failed render_all_pages");
    assert_eq!(all_buffers.len(), 2);
    assert_eq!(all_buffers[0].page_index, 0);
    assert_eq!(all_buffers[1].page_index, 1);
}

#[test]
fn test_page_raster_buffer_crop_region() {
    let pdf_bytes = generate_test_pdf_bytes("<h1>Cropping Test</h1><p>Paragraph to be cropped</p>");
    let rasterizer = TinySkiaRasterizer::new();
    let buffer = rasterizer.render_page(&pdf_bytes, 0, 150).unwrap();

    let bbox = BoundingBox::from_coords(0.1, 0.1, 0.6, 0.6).unwrap();
    let cropped = buffer.crop_region(&bbox).expect("Failed crop_region");

    let expected_rect = bbox.to_pixel_rect(buffer.width(), buffer.height());
    assert_eq!(cropped.width(), expected_rect.width);
    assert_eq!(cropped.height(), expected_rect.height);
}

#[test]
fn test_page_raster_buffer_crop_to_image_crop() {
    let pdf_bytes = generate_test_pdf_bytes("<h1>Image Crop Test</h1>");
    let rasterizer = TinySkiaRasterizer::new();
    let buffer = rasterizer.render_page(&pdf_bytes, 0, 150).unwrap();

    let bbox = BoundingBox::from_coords(0.2, 0.2, 0.5, 0.5).unwrap();

    // Test PNG crop
    let png_crop = buffer
        .crop_to_image_crop(&bbox, CropFormat::Png)
        .expect("Failed PNG crop");
    assert_eq!(png_crop.mime_type, "image/png");
    assert_eq!(png_crop.page_index, 0);
    assert!(!png_crop.encoded_data.is_empty());
    assert!(png_crop.width > 0);
    assert!(png_crop.height > 0);

    // Test JPEG crop
    let jpeg_crop = buffer
        .crop_to_image_crop(&bbox, CropFormat::default_jpeg())
        .expect("Failed JPEG crop");
    assert_eq!(jpeg_crop.mime_type, "image/jpeg");
    assert!(!jpeg_crop.encoded_data.is_empty());

    // Test Raw RGBA crop
    let raw_crop = buffer
        .crop_to_image_crop(&bbox, CropFormat::RawRgba)
        .expect("Failed RawRgba crop");
    assert_eq!(raw_crop.mime_type, "image/x-raw-rgba");
    assert!(!raw_crop.encoded_data.is_empty());
}

#[test]
fn test_pdf_region_cropper_integration() {
    let pdf_bytes = generate_test_pdf_bytes("<h1>Region Cropper Integration</h1>");
    let cropper = PdfRegionCropper::new(&pdf_bytes).with_dpi(150);

    // Full page capture
    let page_crop = cropper
        .capture_page(0, 150, CropFormat::Png)
        .expect("Failed capture_page");
    assert_eq!(page_crop.page_index, 0);
    assert_eq!(page_crop.mime_type, "image/png");

    // Sub-region crop
    let bbox = BoundingBox::from_coords(0.1, 0.1, 0.4, 0.4).unwrap();
    let sub_crop = cropper
        .crop_region(0, &bbox, 0.05, CropFormat::Png)
        .expect("Failed crop_region");
    assert_eq!(sub_crop.page_index, 0);
    assert!(sub_crop.width > 0);
    assert!(sub_crop.height > 0);
}
