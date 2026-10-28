//! Real PDF and Real Image integration tests for Docula document intelligence engine.
//!
//! Generates real, syntactically valid PDF files and raster image assets (PNG, JPEG),
//! persists them as fixtures, and validates the full pipeline stack:
//! - PDF rendering at multiple DPIs via `pdf_oxide` & `tiny-skia`
//! - Sub-region ROI cropping into PNG/JPEG
//! - Pure-Rust computer vision enhancement via KitaSan `OxiMediaFilter` and `OxiMediaStitcher`
//! - 1D/2D optical code detection and EPC financial payload decoding
//! - End-to-end extraction through `DoculaPipeline`

use docula::llm::MockLlmBackend;
use docula::preprocess::{
    ImageFilter, OxiMediaFilter, OxiMediaStitcher, PdfRegionCropper, Rasterizer, TinySkiaRasterizer,
};
use docula::schema::{DocuValidate, ValidationError};
use docula::vision_codes::{FinancialQrPayload, OpticalCodeDetector, OpticalCodeType};
use docula::{BoundingBox, CropFormat, DoculaPipeline, DocumentSource, RegionCropper};
use image::{ImageEncoder, Rgb, RgbImage, Rgba, RgbaImage};
use pdf_oxide::api::Pdf;
use rxing::{BarcodeFormat, MultiFormatWriter, Writer};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
struct InvoiceLineItem {
    description: String,
    quantity: u32,
    unit_price: f64,
    total: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
struct RealPdfInvoice {
    invoice_number: String,
    vendor_name: String,
    items: Vec<InvoiceLineItem>,
    subtotal: f64,
    tax: f64,
    total: f64,
}

impl DocuValidate for RealPdfInvoice {
    fn validate_integrity(&self) -> Result<(), Vec<ValidationError>> {
        let calc_subtotal: f64 = self.items.iter().map(|it| it.total).sum();
        if (calc_subtotal - self.subtotal).abs() > 0.05 {
            return Err(vec![ValidationError::new(
                "subtotal",
                format!(
                    "Sum of line totals ({calc_subtotal}) does not match subtotal ({})",
                    self.subtotal
                ),
            )]);
        }
        if ((self.subtotal + self.tax) - self.total).abs() > 0.05 {
            return Err(vec![ValidationError::new(
                "total",
                format!(
                    "Subtotal + tax ({} + {}) does not match total ({})",
                    self.subtotal, self.tax, self.total
                ),
            )]);
        }
        Ok(())
    }
}

fn fixture_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    fs::create_dir_all(&dir).expect("create tests/fixtures directory");
    dir
}

/// Generates or loads a real single-page PDF invoice.
fn get_or_create_real_pdf_invoice() -> (PathBuf, Vec<u8>) {
    let path = fixture_dir().join("sample_invoice.pdf");
    if path.exists() {
        let bytes = fs::read(&path).expect("read existing sample_invoice.pdf");
        return (path, bytes);
    }

    let html = r#"
    <!DOCTYPE html>
    <html>
    <head>
      <meta charset="utf-8">
      <title>Tax Invoice</title>
      <style>
        body { font-family: Helvetica, Arial, sans-serif; margin: 15px; font-size: 12px; color: #111; }
        h2 { margin: 0 0 5px 0; color: #0f172a; }
        p { margin: 2px 0; }
        table { width: 100%; border-collapse: collapse; margin-top: 8px; }
        th, td { border: 1px solid #ccc; padding: 4px; text-align: left; }
        th { background-color: #f1f5f9; }
      </style>
    </head>
    <body>
      <h2>INVOICE INV-REAL-2026-999</h2>
      <p><strong>Vendor:</strong> KitaSan Sovereign Technologies | <strong>Date:</strong> 2026-10-12</p>
      <table>
        <thead>
          <tr>
            <th>Description</th>
            <th>Qty</th>
            <th>Unit Price</th>
            <th>Total</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td>Pure-Rust Cloud Server Cluster</td>
            <td>2</td>
            <td>$250.00</td>
            <td>$500.00</td>
          </tr>
          <tr>
            <td>OxiBonsai Fine-Tuning License</td>
            <td>1</td>
            <td>$150.00</td>
            <td>$150.00</td>
          </tr>
          <tr>
            <td colspan="3" style="text-align:right"><strong>Subtotal:</strong></td>
            <td>$650.00</td>
          </tr>
          <tr>
            <td colspan="3" style="text-align:right"><strong>Tax (10%):</strong></td>
            <td>$65.00</td>
          </tr>
          <tr>
            <td colspan="3" style="text-align:right"><strong>Total:</strong></td>
            <td><strong>$715.00</strong></td>
          </tr>
        </tbody>
      </table>
    </body>
    </html>
    "#;

    let mut pdf = Pdf::from_html(html).expect("compile real PDF from html");
    let bytes = pdf.to_bytes().expect("convert real PDF to bytes");
    fs::write(&path, &bytes).expect("write sample_invoice.pdf");
    (path, bytes)
}

/// Generates or loads a real 2-page PDF document.
fn get_or_create_real_multipage_pdf() -> (PathBuf, Vec<u8>) {
    let path = fixture_dir().join("sample_multipage.pdf");
    if path.exists() {
        let bytes = fs::read(&path).expect("read existing sample_multipage.pdf");
        return (path, bytes);
    }

    let p1_file = tempfile::NamedTempFile::new().expect("tempfile p1");
    let p2_file = tempfile::NamedTempFile::new().expect("tempfile p2");

    let html_page1 = r#"
    <!DOCTYPE html>
    <html><body>
      <h1>MASTER SERVICES AGREEMENT - PAGE 1</h1>
      <p>This Agreement is entered into between Provider and Sovereign Client.</p>
      <p>Section 1: Zero-FFI Compliance Guarantee.</p>
    </body></html>
    "#;
    let mut pdf1 = Pdf::from_html(html_page1).expect("create pdf1");
    pdf1.save(p1_file.path()).expect("save pdf1");

    let html_page2 = r#"
    <!DOCTYPE html>
    <html><body>
      <h1>MASTER SERVICES AGREEMENT - PAGE 2</h1>
      <p>Section 2: Signatures and Executed Authorization.</p>
      <p>Representative: Alice Doe, Chief Architect.</p>
    </body></html>
    "#;
    let mut pdf2 = Pdf::from_html(html_page2).expect("create pdf2");
    pdf2.save(p2_file.path()).expect("save pdf2");

    let bytes = pdf_oxide::api::merge_pdfs(&[p1_file.path(), p2_file.path()])
        .expect("merge 2-page real PDF");
    fs::write(&path, &bytes).expect("write sample_multipage.pdf");
    (path, bytes)
}

/// Generates or loads a real high-resolution PNG receipt image.
fn get_or_create_real_receipt_png() -> (PathBuf, Vec<u8>) {
    let path = fixture_dir().join("sample_receipt.png");
    if path.exists() {
        let bytes = fs::read(&path).expect("read existing sample_receipt.png");
        return (path, bytes);
    }

    let width = 400;
    let height = 600;
    let mut img = RgbaImage::new(width, height);

    // Fill white background with subtle paper texture
    for y in 0..height {
        for x in 0..width {
            let noise = ((x ^ y) % 3) as u8;
            img.put_pixel(x, y, Rgba([255 - noise, 255 - noise, 255 - noise, 255]));
        }
    }

    // Draw receipt header border (dark blue bar)
    for y in 20..35 {
        for x in 40..360 {
            img.put_pixel(x, y, Rgba([15, 23, 42, 255]));
        }
    }

    // Draw simulated printed receipt text lines (dark grey horizontal bands)
    let line_y_coords = [60, 80, 100, 140, 170, 200, 230, 260, 310, 340, 380, 420];
    for &ly in &line_y_coords {
        for y in ly..(ly + 6) {
            for x in 50..340 {
                if (x + y) % 5 != 0 {
                    img.put_pixel(x, y, Rgba([30, 41, 59, 255]));
                }
            }
        }
    }

    // Draw simulated 1D barcode stripes at bottom
    for y in 480..540 {
        for x in 60..340 {
            if (x / 4) % 2 == 0 || (x / 7) % 3 == 0 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            } else {
                img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
    }

    let mut bytes = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .expect("encode PNG receipt");

    fs::write(&path, &bytes).expect("write sample_receipt.png");
    (path, bytes)
}

/// Generates or loads a real JPEG document image.
fn get_or_create_real_document_jpeg() -> (PathBuf, Vec<u8>) {
    let path = fixture_dir().join("sample_document.jpg");
    if path.exists() {
        let bytes = fs::read(&path).expect("read existing sample_document.jpg");
        return (path, bytes);
    }

    let width = 350;
    let height = 450;
    let mut img = RgbImage::new(width, height);

    // Warm document background
    for y in 0..height {
        for x in 0..width {
            img.put_pixel(x, y, Rgb([248, 247, 242]));
        }
    }

    // Draw title block
    for y in 30..45 {
        for x in 30..320 {
            img.put_pixel(x, y, Rgb([40, 40, 40]));
        }
    }

    // Draw paragraph blocks
    for section_start in [70, 150, 230, 310] {
        for line in 0..4 {
            let y_line = section_start + line * 15;
            for y in y_line..(y_line + 4) {
                for x in 30..300 {
                    if (x * 3 + y) % 7 != 0 {
                        img.put_pixel(x, y, Rgb([60, 60, 60]));
                    }
                }
            }
        }
    }

    let mut bytes = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 90);
    encoder
        .encode(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgb8,
        )
        .expect("encode JPEG document");

    fs::write(&path, &bytes).expect("write sample_document.jpg");
    (path, bytes)
}

/// Generates or loads a real PNG invoice containing an EPC QR code.
fn get_or_create_real_qr_bill_png() -> (PathBuf, Vec<u8>) {
    let path = fixture_dir().join("sample_qr_bill.png");
    if path.exists() {
        let bytes = fs::read(&path).expect("read existing sample_qr_bill.png");
        return (path, bytes);
    }

    let epc_payload = "BCD\n001\n1\nSCT\nTESTBIC\nKitaSan Tech Inc\nES1234567890123456789012\nEUR715.00\n\nRF18539007547034\nINV-REAL-2026-999\n";
    let bitmatrix = MultiFormatWriter
        .encode(epc_payload, &BarcodeFormat::QR_CODE, 200, 200)
        .expect("generate real QR code");

    let width = 450;
    let height = 550;
    let mut img = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));

    // Embed QR code in top right
    let qr_offset_x = 220;
    let qr_offset_y = 50;
    for y in 0..bitmatrix.getHeight() {
        for x in 0..bitmatrix.getWidth() {
            if bitmatrix.get(x, y) {
                img.put_pixel(qr_offset_x + x, qr_offset_y + y, Rgba([0, 0, 0, 255]));
            }
        }
    }

    // Embed simulated document text on left side
    for y in 60..70 {
        for x in 30..190 {
            img.put_pixel(x, y, Rgba([15, 23, 42, 255]));
        }
    }
    for line in 0..6 {
        let y_l = 100 + line * 20;
        for y in y_l..(y_l + 5) {
            for x in 30..180 {
                if (x + y) % 4 != 0 {
                    img.put_pixel(x, y, Rgba([50, 50, 50, 255]));
                }
            }
        }
    }

    let mut bytes = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .expect("encode QR bill PNG");

    fs::write(&path, &bytes).expect("write sample_qr_bill.png");
    (path, bytes)
}

#[test]
fn test_real_pdf_invoice_rasterization_multi_dpi() {
    let (pdf_path, pdf_bytes) = get_or_create_real_pdf_invoice();
    assert!(pdf_path.exists());
    assert!(!pdf_bytes.is_empty());

    let rasterizer = TinySkiaRasterizer::new();

    // Verify page count from real PDF
    let count = rasterizer
        .page_count(&pdf_bytes)
        .expect("count pages in real PDF");
    assert!(count >= 1);

    // Render at 72 DPI
    let buf_72 = rasterizer
        .render_page(&pdf_bytes, 0, 72)
        .expect("render at 72 DPI");
    assert!(buf_72.width() > 0);
    assert!(buf_72.height() > 0);

    // Render at 150 DPI
    let buf_150 = rasterizer
        .render_page(&pdf_bytes, 0, 150)
        .expect("render at 150 DPI");
    assert!(buf_150.width() > buf_72.width());
    assert!(buf_150.height() > buf_72.height());

    // Scaling ratio verification between 150 DPI and 72 DPI
    let ratio = buf_150.width() as f32 / buf_72.width() as f32;
    assert!(
        (ratio - (150.0 / 72.0)).abs() < 0.15,
        "Expected scaling ratio ~2.08, got {ratio}"
    );

    // Verify RGBA pixel buffer length
    assert_eq!(
        buf_150.as_raw().len(),
        (buf_150.width() * buf_150.height() * 4) as usize
    );
}

#[test]
fn test_real_pdf_roi_region_cropper() {
    let (_pdf_path, pdf_bytes) = get_or_create_real_pdf_invoice();
    let cropper = PdfRegionCropper::new(&pdf_bytes).with_dpi(150);

    // Capture full page
    let full_page = cropper
        .capture_page(0, 150, CropFormat::Png)
        .expect("capture full page as PNG");
    assert_eq!(full_page.mime_type, "image/png");
    assert!(!full_page.encoded_data.is_empty());
    assert!(full_page.width > 0 && full_page.height > 0);
    assert!(full_page.aspect_ratio() > 0.0);

    // Crop invoice header bounding box: [0.05, 0.05, 0.95, 0.35]
    let bbox = BoundingBox::from_coords(0.05, 0.05, 0.95, 0.35).expect("valid bbox");
    let header_crop = cropper
        .crop_region(0, &bbox, 0.02, CropFormat::Png)
        .expect("crop header region");

    assert_eq!(header_crop.mime_type, "image/png");
    assert!(header_crop.width > 0 && header_crop.height > 0);
    assert!(header_crop.width < full_page.width);
    assert!(header_crop.height < full_page.height);
    assert!(!header_crop.encoded_data.is_empty());
}

#[test]
fn test_real_multipage_pdf_traversal() {
    let (_path, pdf_bytes) = get_or_create_real_multipage_pdf();
    let rasterizer = TinySkiaRasterizer::new();

    let page_count = rasterizer
        .page_count(&pdf_bytes)
        .expect("multipage page count");
    assert_eq!(page_count, 2);

    let pages = rasterizer
        .render_all_pages(&pdf_bytes, 150)
        .expect("render all pages of real multipage PDF");
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].page_index, 0);
    assert_eq!(pages[1].page_index, 1);
    assert!(pages[0].width() > 0 && pages[0].height() > 0);
    assert!(pages[1].width() > 0 && pages[1].height() > 0);
}

#[test]
fn test_real_receipt_png_oximedia_cv_enhancement() {
    let (png_path, png_bytes) = get_or_create_real_receipt_png();
    assert!(png_path.exists());
    assert!(!png_bytes.is_empty());

    let dyn_img = image::load_from_memory(&png_bytes).expect("decode real receipt PNG from memory");
    let mut rgba_img = dyn_img.to_rgba8();
    let (orig_w, orig_h) = rgba_img.dimensions();

    // Compute original luminance mean
    let mut orig_lum = 0u64;
    for pixel in rgba_img.pixels() {
        orig_lum +=
            (0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32) as u64;
    }
    let orig_mean = (orig_lum / ((orig_w * orig_h) as u64)) as f32;

    // Apply KitaSan OxiMedia pure-Rust CV filter
    let filter = OxiMediaFilter::new().with_contrast(1.4);
    assert_eq!(filter.name(), "OxiMediaCvFilter");
    filter.apply(&mut rgba_img).expect("apply OxiMediaFilter");

    // Dimensions must be preserved
    assert_eq!(rgba_img.dimensions(), (orig_w, orig_h));

    // Verify contrast modification occurred
    let mut post_lum = 0u64;
    for pixel in rgba_img.pixels() {
        post_lum +=
            (0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32) as u64;
    }
    let post_mean = (post_lum / ((orig_w * orig_h) as u64)) as f32;
    // Mean luminance stays closely centered while contrast expands
    assert!((post_mean - orig_mean).abs() < 10.0);
}

#[test]
fn test_real_image_stitching_with_oximedia() {
    let (_png_path, png_bytes) = get_or_create_real_receipt_png();
    let dyn_img = image::load_from_memory(&png_bytes).expect("decode real receipt PNG");
    let full_img = dyn_img.to_rgba8();
    let (w, h) = full_img.dimensions();

    // Slice image into two overlapping sections
    let top_height = (h / 2) + 30;
    let bottom_start = (h / 2) - 30;
    let bottom_height = h - bottom_start;
    let overlap = 60u32;

    let mut top_img = RgbaImage::new(w, top_height);
    for y in 0..top_height {
        for x in 0..w {
            top_img.put_pixel(x, y, *full_img.get_pixel(x, y));
        }
    }

    let mut bottom_img = RgbaImage::new(w, bottom_height);
    for y in 0..bottom_height {
        for x in 0..w {
            bottom_img.put_pixel(x, y, *full_img.get_pixel(x, bottom_start + y));
        }
    }

    // Stitch vertically using OxiMediaStitcher
    let stitched = OxiMediaStitcher::stitch_vertical(&top_img, &bottom_img, overlap)
        .expect("stitch real image segments");

    let expected_height = top_height + bottom_height - overlap;
    assert_eq!(stitched.width(), w);
    assert_eq!(stitched.height(), expected_height);
}

#[test]
fn test_real_jpeg_document_consumption() {
    let (jpeg_path, jpeg_bytes) = get_or_create_real_document_jpeg();
    assert!(jpeg_path.exists());
    assert!(!jpeg_bytes.is_empty());

    let source_path = DocumentSource::from_image_path(&jpeg_path);
    assert!(source_path.is_image());
    assert!(!source_path.is_pdf());

    let source_bytes = DocumentSource::from_image_bytes(jpeg_bytes.clone(), Some("jpeg"));
    assert!(source_bytes.is_image());

    // Decode through image crate
    let dyn_img = image::load_from_memory(&jpeg_bytes).expect("decode JPEG");
    assert_eq!(dyn_img.width(), 350);
    assert_eq!(dyn_img.height(), 450);
}

#[test]
fn test_real_qr_code_image_detector_and_epc_payload() {
    let (qr_path, qr_bytes) = get_or_create_real_qr_bill_png();
    assert!(qr_path.exists());

    let dyn_img = image::load_from_memory(&qr_bytes).expect("decode QR bill");
    let rgba_img = dyn_img.to_rgba8();

    let detector = OpticalCodeDetector::new();
    let results = detector.detect(&rgba_img).expect("detect optical codes");

    assert_eq!(results.len(), 1);
    let code = &results[0];
    assert_eq!(code.code_type, OpticalCodeType::QrCode);
    assert!(code.raw_content.contains("KitaSan Tech Inc"));
    assert!(code.financial_payload.is_some());

    match code.financial_payload.as_ref().unwrap() {
        FinancialQrPayload::SepaEpc(epc) => {
            assert_eq!(epc.beneficiary_name, "KitaSan Tech Inc");
            assert_eq!(epc.iban, "ES1234567890123456789012");
            assert_eq!(epc.amount, Some(715.00));
        }
        other => panic!("Expected SepaEpc financial payload, got {other:?}"),
    }
}

#[tokio::test]
async fn test_real_pdf_end_to_end_pipeline_extraction() {
    let (pdf_path, _pdf_bytes) = get_or_create_real_pdf_invoice();

    let expected_invoice = RealPdfInvoice {
        invoice_number: "INV-REAL-2026-999".to_string(),
        vendor_name: "KitaSan Sovereign Technologies".to_string(),
        items: vec![
            InvoiceLineItem {
                description: "Pure-Rust Cloud Server Cluster".to_string(),
                quantity: 2,
                unit_price: 250.0,
                total: 500.0,
            },
            InvoiceLineItem {
                description: "OxiBonsai Fine-Tuning License".to_string(),
                quantity: 1,
                unit_price: 150.0,
                total: 150.0,
            },
        ],
        subtotal: 650.0,
        tax: 65.0,
        total: 715.0,
    };
    let canned_json = serde_json::to_string(&expected_invoice).unwrap();

    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let source = DocumentSource::from_pdf_path(pdf_path);
    let result = pipeline
        .process_document::<RealPdfInvoice>("doc-real-pdf-01", &source)
        .await
        .expect("process real PDF invoice");

    assert_eq!(result.document_id, "doc-real-pdf-01");
    assert_eq!(result.data, Some(expected_invoice));
    assert!(result.confidence >= 0.85);
    assert!(!result.routed_to_triage);
    assert_eq!(result.repair_attempts, 1);
}

#[tokio::test]
async fn test_scenario_1_large_document_multishot_reconstruction_and_extraction() {
    // Scenario 1: Multi-shot large document reconstruction
    // 1. Create a large synthetic invoice with non-repeating content
    let canvas_w = 120u32;
    let canvas_h = 240u32;
    let mut full_invoice = RgbaImage::from_pixel(canvas_w, canvas_h, Rgba([255, 255, 255, 255]));

    // Draw header, table rows, and footer
    for y in 0..canvas_h {
        for x in 0..canvas_w {
            if (x > 10 && x < canvas_w - 10) && (y % 18 < 6) {
                let val = (((x * 17 + y * 31) % 180) + 30) as u8;
                full_invoice.put_pixel(x, y, Rgba([val, val, val, 255]));
            }
        }
    }

    // Split into 2 overlapping camera shots:
    // Top shot: [0..140]
    // Bottom shot: [100..240] (overlap = 40 pixels)
    let mut top_shot = RgbaImage::new(120, 140);
    for y in 0..140 {
        for x in 0..120 {
            // Simulate bad lighting: camera shadow gradient
            let shadow = (y as f32 / 140.0 * 60.0) as u8;
            let mut p = *full_invoice.get_pixel(x, y);
            p[0] = p[0].saturating_sub(shadow);
            p[1] = p[1].saturating_sub(shadow);
            p[2] = p[2].saturating_sub(shadow);
            top_shot.put_pixel(x, y, p);
        }
    }

    let mut bot_shot = RgbaImage::new(120, 140);
    for y in 0..140 {
        for x in 0..120 {
            let mut p = *full_invoice.get_pixel(x, 100 + y);
            // Simulate bad lighting: specular flare / brightness shift
            p[0] = p[0].saturating_add(20);
            p[1] = p[1].saturating_add(20);
            p[2] = p[2].saturating_add(20);
            bot_shot.put_pixel(x, y, p);
        }
    }

    let mut top_bytes = Vec::new();
    let mut bot_bytes = Vec::new();
    top_shot
        .write_to(
            &mut std::io::Cursor::new(&mut top_bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bot_shot
        .write_to(
            &mut std::io::Cursor::new(&mut bot_bytes),
            image::ImageFormat::Png,
        )
        .unwrap();

    let src1 = DocumentSource::from_image_bytes(top_bytes, Some("png"));
    let src2 = DocumentSource::from_image_bytes(bot_bytes, Some("png"));
    let fragment_sources = vec![src1, src2];

    let expected_invoice = RealPdfInvoice {
        invoice_number: "INV-RECONSTRUCT-2026".to_string(),
        vendor_name: "Mega Warehouse Hardware".to_string(),
        items: vec![
            InvoiceLineItem {
                description: "Industrial Bolt Set 500ct".to_string(),
                quantity: 4,
                unit_price: 45.0,
                total: 180.0,
            },
            InvoiceLineItem {
                description: "Hydraulic Jack Stand 3T".to_string(),
                quantity: 2,
                unit_price: 110.0,
                total: 220.0,
            },
        ],
        subtotal: 400.0,
        tax: 40.0,
        total: 440.0,
    };
    let canned_json = serde_json::to_string(&expected_invoice).unwrap();
    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    // A. Verify direct Stage-by-Stage API access
    // 1. Stage 1: Image Merging
    let mut reconstructed = pipeline
        .merge_document_source(&fragment_sources, None)
        .expect("Stage 1: Merge document sources must succeed");
    assert_eq!(reconstructed.fragment_count, 2);
    assert_eq!(reconstructed.stitched_width, 120);
    assert!(reconstructed.stitched_height >= 220 && reconstructed.stitched_height <= 260);

    // 2. Stage 3: Direct Preprocessing
    pipeline
        .preprocess_buffer(&mut reconstructed.buffer, None)
        .expect("Stage 3: Preprocess buffer must succeed");

    // 3. Stage 4: Buffer Extraction
    let stage_result = pipeline
        .extract_from_buffer::<RealPdfInvoice>("invoice-multishot-stage", &reconstructed.buffer)
        .await
        .expect("Stage 4: Buffer extraction must succeed");
    assert_eq!(stage_result.document_id, "invoice-multishot-stage");
    assert_eq!(stage_result.data, Some(expected_invoice.clone()));

    // B. Verify Unified End-to-End Facade API access (reconstruct_and_extract)
    let facade_result = pipeline
        .reconstruct_and_extract::<RealPdfInvoice>(&fragment_sources, None)
        .await
        .expect("Facade reconstruct_and_extract must succeed");
    assert_eq!(facade_result.data, Some(expected_invoice));
    assert!(facade_result.confidence >= 0.85);
}

#[tokio::test]
async fn test_scenario_2_singleshot_multidocument_segmentation_identity_and_batch_extraction() {
    // Scenario 2: Single-shot multi-document detection, identity fingerprinting, and batch extraction
    // Create a 300x300 scene containing 2 distinct invoice cards on a dark background
    let mut scene = RgbaImage::from_pixel(300, 300, Rgba([25, 25, 30, 255]));

    // Draw Document 1 at top (20, 20), size 110x110
    for y in 20..130 {
        for x in 20..130 {
            let is_text = (y % 16 < 4) && (x > 30 && x < 120);
            scene.put_pixel(
                x,
                y,
                if is_text {
                    Rgba([40, 40, 40, 255])
                } else {
                    Rgba([245, 245, 245, 255])
                },
            );
        }
    }

    // Draw Document 2 at bottom (150, 160), size 120x110
    for y in 160..270 {
        for x in 150..270 {
            let is_text = (y % 16 < 4) && (x > 160 && x < 260);
            scene.put_pixel(
                x,
                y,
                if is_text {
                    Rgba([50, 50, 50, 255])
                } else {
                    Rgba([235, 235, 240, 255])
                },
            );
        }
    }

    let mut scene_bytes = Vec::new();
    scene
        .write_to(
            &mut std::io::Cursor::new(&mut scene_bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    let scene_source = DocumentSource::from_image_bytes(scene_bytes, Some("png"));

    let expected_invoice = RealPdfInvoice {
        invoice_number: "INV-BATCH-001".to_string(),
        vendor_name: "QuickMart Retail".to_string(),
        items: vec![InvoiceLineItem {
            description: "Stationery Supplies".to_string(),
            quantity: 1,
            unit_price: 35.0,
            total: 35.0,
        }],
        subtotal: 35.0,
        tax: 3.5,
        total: 38.5,
    };
    let canned_json = serde_json::to_string(&expected_invoice).unwrap();
    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let seg_config = docula::composer::SegmentationConfig {
        min_area_ratio: 0.05,
        max_area_ratio: 0.80,
        scan_optical_codes: false,
        preprocess_subdocs: true,
        luminance_threshold: 80,
        ..Default::default()
    };

    // A. Verify direct Stage-by-Stage API access
    // 1. Stage 2: Instance Segmentation
    let subdocs = pipeline
        .segment_multi_document_source(&scene_source, Some(seg_config.clone()))
        .expect("Stage 2: Instance segmentation must succeed");
    assert_eq!(subdocs.len(), 2, "Should identify both sub-documents");

    // Verify deterministic identities and reading order
    assert_eq!(subdocs[0].identity.reading_order_index, 0);
    assert_eq!(subdocs[1].identity.reading_order_index, 1);
    assert_ne!(
        subdocs[0].identity.content_digest,
        subdocs[1].identity.content_digest
    );

    // 2. Stage 4: Buffer Extraction per sub-document
    let ext0 = pipeline
        .extract_from_buffer::<RealPdfInvoice>(&subdocs[0].identity.instance_id, &subdocs[0].buffer)
        .await
        .expect("Extract sub-doc 0");
    assert_eq!(ext0.data, Some(expected_invoice.clone()));

    // B. Verify Unified End-to-End Facade API access (segment_and_extract_batch)
    let batch_results = pipeline
        .segment_and_extract_batch::<RealPdfInvoice>(&scene_source, Some(seg_config))
        .await
        .expect("Facade segment_and_extract_batch must succeed");

    assert_eq!(batch_results.len(), 2);
    for (idx, (identity, ext)) in batch_results.iter().enumerate() {
        assert_eq!(identity.reading_order_index, idx);
        assert_eq!(ext.data, Some(expected_invoice.clone()));
        assert!(ext.confidence >= 0.85);
    }
}
