use docula_core::crop::{CropFormat, ImageCrop, RegionCropper};
use docula_core::error::{CropError, DoculaError};
use docula_core::geometry::BoundingBox;
use docula_core::source::DocumentSource;
use std::path::{Path, PathBuf};

// Mock RegionCropper implementation to verify Strategy trait ergonomics
struct MockCropper {
    total_pages: usize,
}

impl RegionCropper for MockCropper {
    fn capture_page(
        &self,
        page_index: usize,
        _dpi: u32,
        format: CropFormat,
    ) -> Result<ImageCrop, DoculaError> {
        if page_index >= self.total_pages {
            return Err(CropError::PageOutOfBounds {
                page_index,
                total_pages: self.total_pages,
            }
            .into());
        }

        let bbox = BoundingBox::from_coords(0.0, 0.0, 1.0, 1.0)?;
        let crop = ImageCrop::new(
            page_index,
            bbox,
            1920,
            1080,
            "mock_base64_data",
            format.mime_type(),
        )?;
        Ok(crop)
    }

    fn crop_region(
        &self,
        page_index: usize,
        bbox: &BoundingBox,
        padding: f32,
        format: CropFormat,
    ) -> Result<ImageCrop, DoculaError> {
        if page_index >= self.total_pages {
            return Err(CropError::PageOutOfBounds {
                page_index,
                total_pages: self.total_pages,
            }
            .into());
        }

        let padded_bbox = bbox.with_padding(padding);
        let pixel_rect = padded_bbox.to_pixel_rect(1920, 1080);
        let crop = ImageCrop::new(
            page_index,
            padded_bbox,
            pixel_rect.width,
            pixel_rect.height,
            "mock_region_data",
            format.mime_type(),
        )?;
        Ok(crop)
    }
}

#[test]
fn test_crop_format_attributes() {
    let png = CropFormat::Png;
    assert_eq!(png.mime_type(), "image/png");
    assert_eq!(png.file_extension(), "png");

    let jpeg = CropFormat::Jpeg { quality: 90 };
    assert_eq!(jpeg.mime_type(), "image/jpeg");
    assert_eq!(jpeg.file_extension(), "jpg");

    let webp = CropFormat::WebP;
    assert_eq!(webp.mime_type(), "image/webp");
    assert_eq!(webp.file_extension(), "webp");

    let raw = CropFormat::RawRgba;
    assert_eq!(raw.mime_type(), "image/x-raw-rgba");
    assert_eq!(raw.file_extension(), "rgba");
}

#[test]
fn test_image_crop_creation_and_serde() {
    let bbox = BoundingBox::from_coords(0.1, 0.2, 0.5, 0.6).unwrap();
    let crop = ImageCrop::new(1, bbox, 400, 300, "sample_base64_payload", "image/png")
        .expect("valid image crop");

    assert_eq!(crop.page_index, 1);
    assert_eq!(crop.width, 400);
    assert_eq!(crop.height, 300);
    assert!((crop.aspect_ratio() - (400.0 / 300.0)).abs() < 1e-5);
    assert_eq!(crop.payload_len(), "sample_base64_payload".len());

    // Serde roundtrip
    let serialized = serde_json::to_string(&crop).expect("serialization succeeds");
    let deserialized: ImageCrop =
        serde_json::from_str(&serialized).expect("deserialization succeeds");
    assert_eq!(crop, deserialized);

    // Zero dimensions error
    let zero_crop = ImageCrop::new(0, bbox, 0, 300, "data", "image/png");
    assert!(matches!(zero_crop, Err(CropError::ZeroDimensions { .. })));
}

#[test]
fn test_region_cropper_mock_contract() {
    let cropper = MockCropper { total_pages: 3 };

    // Valid full-page capture
    let page_0 = cropper
        .capture_page(0, 300, CropFormat::Png)
        .expect("page capture succeeds");
    assert_eq!(page_0.page_index, 0);
    assert_eq!(page_0.width, 1920);
    assert_eq!(page_0.height, 1080);
    assert_eq!(page_0.mime_type, "image/png");

    // Out of bounds page capture
    let oob_res = cropper.capture_page(5, 300, CropFormat::Png);
    assert!(matches!(
        oob_res,
        Err(DoculaError::Crop(CropError::PageOutOfBounds { .. }))
    ));

    // Valid region crop with 5% padding
    let target_bbox = BoundingBox::from_coords(0.2, 0.2, 0.4, 0.4).unwrap();
    let region = cropper
        .crop_region(1, &target_bbox, 0.05, CropFormat::default_jpeg())
        .expect("region crop succeeds");
    assert_eq!(region.page_index, 1);
    assert_eq!(region.mime_type, "image/jpeg");
    assert!(region.width > 0);
    assert!(region.height > 0);
}

#[test]
fn test_document_source_from_bytes() {
    let dummy_pdf = b"%PDF-1.7 sample content".to_vec();
    let source = DocumentSource::from_pdf_bytes(dummy_pdf.clone());

    assert!(source.is_pdf());
    assert!(!source.is_image());
    assert!(!source.is_batch());
    assert_eq!(source.as_bytes(), Some(dummy_pdf.as_slice()));
    assert_eq!(source.byte_len(), Some(dummy_pdf.len()));
    assert_eq!(source.as_path(), None);
}

#[test]
fn test_document_source_from_paths() {
    let pdf_path = PathBuf::from("/tmp/invoice.pdf");
    let source_pdf = DocumentSource::from_pdf_path(pdf_path.clone());
    assert!(source_pdf.is_pdf());
    assert_eq!(source_pdf.as_path(), Some(pdf_path.as_path()));
    assert_eq!(source_pdf.as_bytes(), None);

    let img_path = PathBuf::from("/tmp/receipt.png");
    let source_img = DocumentSource::from_image_path(img_path.clone());
    assert!(source_img.is_image());
    assert!(!source_img.is_pdf());
    assert_eq!(source_img.as_path(), Some(img_path.as_path()));
}

#[test]
fn test_document_source_image_bytes() {
    let img_bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let source = DocumentSource::from_image_bytes(img_bytes.clone(), Some("png"));

    assert!(source.is_image());
    assert!(!source.is_pdf());
    assert_eq!(source.as_bytes(), Some(img_bytes.as_slice()));
    assert_eq!(source.byte_len(), Some(img_bytes.len()));
}

#[test]
fn test_document_source_batch_files() {
    let paths = vec![
        PathBuf::from("/tmp/page1.jpg"),
        PathBuf::from("/tmp/page2.jpg"),
    ];
    let source = DocumentSource::from_batch_files(paths.clone());

    assert!(source.is_batch());
    assert!(!source.is_pdf());
    assert_eq!(source.as_bytes(), None);
}

#[test]
fn test_document_source_from_traits() {
    let bytes = b"hello world".to_vec();
    let source_bytes: DocumentSource = bytes.clone().into();
    assert_eq!(source_bytes.as_bytes(), Some(bytes.as_slice()));

    let pdf_path = PathBuf::from("doc.pdf");
    let source_path: DocumentSource = pdf_path.into();
    assert!(source_path.is_pdf());

    let img_path = Path::new("scan.jpg");
    let source_img_path: DocumentSource = img_path.into();
    assert!(source_img_path.is_image());
}
