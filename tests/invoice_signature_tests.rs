use docula::llm::MockLlmBackend;
use docula::preprocess::{PageRasterBuffer, VectorizedRegion, VisualRegion};
use docula::schema::{DocuValidate, ValidationError};
use docula::{BoundingBox, CropFormat, DoculaPipeline, DocumentSource, ImageCrop};
use image::GenericImageView;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Merchant details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Merchant {
    pub name: String,
    pub direction: String,
}

/// Billing party details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BillInfo {
    pub username: String,
    pub direction: String,
}

/// Shipping/receiving party details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Receiver {
    pub username: String,
    pub direction: String,
}

/// Individual line item in the invoice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Item {
    pub description: String,
    pub qty: u32,
    pub unit_price: f64,
    pub amount: f64,
}

/// Tax information.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Tax {
    pub name: String,
    pub qty: f64,
}

/// Serialized visual image crop payload adhering to JsonSchema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SerializedCrop {
    pub width: u32,
    pub height: u32,
    pub mime_type: String,
    pub encoded_data: String,
}

impl From<ImageCrop> for SerializedCrop {
    fn from(c: ImageCrop) -> Self {
        Self {
            width: c.width,
            height: c.height,
            mime_type: c.mime_type,
            encoded_data: c.encoded_data,
        }
    }
}

/// Strongly typed signature representation supporting multiple extraction variants.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignaturePayload {
    /// Variant 1: Raw / cropped image section.
    Image {
        crop: SerializedCrop,
    },
    /// Variant 2: Binary mask representation only.
    Mask {
        #[schemars(description = "Base64-encoded binary mask PNG")]
        mask_png_base64: String,
        width: u32,
        height: u32,
    },
    /// Variant 3: Grayscale representation keeping only grayscale luminance.
    Grayscale {
        #[schemars(description = "Base64-encoded grayscale PNG")]
        grayscale_png_base64: String,
        width: u32,
        height: u32,
    },
    /// Variant 4: Vectorized representation containing SVG markup and path contours.
    Vectorized {
        #[schemars(description = "Raw SVG markup of the signature")]
        svg: String,
        path_data: String,
        width: u32,
        height: u32,
    },
}

/// Structured document schema for `invoice_example.png`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Invoice<S = SignaturePayload> {
    pub merchant: Merchant,
    pub bill_info: BillInfo,
    pub receiver: Receiver,
    pub items: Vec<Item>,
    pub taxes: Vec<Tax>,
    pub total: f64,
    pub signature: S,
}

impl<S> Invoice<S> {
    /// Validates that the sum of the amount of all items plus the taxes is equal to the total.
    pub fn validate_amount_plus_tax_equals_total(&self) -> Result<(), ValidationError> {
        let items_sum: f64 = self.items.iter().map(|item| item.amount).sum();
        let taxes_sum: f64 = self.taxes.iter().map(|tax| tax.qty).sum();
        let expected_total = items_sum + taxes_sum;
        let diff = (expected_total - self.total).abs();
        if diff > 0.05 {
            Err(ValidationError::new(
                "total",
                format!(
                    "Sum of items amount ({:.2}) plus taxes ({:.2}) = {:.2} does not equal total ({:.2}) (diff: {:.4})",
                    items_sum, taxes_sum, expected_total, self.total, diff
                ),
            ))
        } else {
            Ok(())
        }
    }

    /// Convenience validation alias.
    #[inline]
    pub fn validate_totals(&self) -> Result<(), ValidationError> {
        self.validate_amount_plus_tax_equals_total()
    }
}

impl<S: Send + Sync + Clone> DocuValidate for Invoice<S> {
    fn validate_integrity(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        if let Err(e) = self.validate_amount_plus_tax_equals_total() {
            errors.push(e);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn fixture_invoice_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join("invoice_example.png")
}

/// Normalized bounding box bounding the "John Smith" handwritten signature in `invoice_example.png`.
fn signature_bounding_box() -> BoundingBox {
    // Exact pixels: x in [540, 715], y in [610, 720] on 750x1061 image
    BoundingBox::from_coords(0.71, 0.57, 0.96, 0.69).expect("valid normalized bbox")
}

/// Helper building the common invoice data for tests.
fn build_base_invoice<S>(signature: S, total: f64) -> Invoice<S> {
    Invoice {
        merchant: Merchant {
            name: "East Repair Inc.".to_string(),
            direction: "1912 Harvest Lane, New York, NY 12210".to_string(),
        },
        bill_info: BillInfo {
            username: "John Smith".to_string(),
            direction: "2 Court Square, New York, NY 12210".to_string(),
        },
        receiver: Receiver {
            username: "John Smith".to_string(),
            direction: "3787 Pineview Drive, Cambridge, MA 12210".to_string(),
        },
        items: vec![
            Item {
                description: "Front and rear brake cables".to_string(),
                qty: 1,
                unit_price: 100.0,
                amount: 100.0,
            },
            Item {
                description: "New set of pedal arms".to_string(),
                qty: 2,
                unit_price: 15.0,
                amount: 30.0,
            },
            Item {
                description: "Labor 3hrs".to_string(),
                qty: 3,
                unit_price: 5.0,
                amount: 15.0,
            },
        ],
        taxes: vec![Tax {
            name: "Sales Tax 6.25%".to_string(),
            qty: 9.06,
        }],
        total,
        signature,
    }
}

#[test]
fn test_invoice_math_validation_success() {
    // Items: 100.0 + 30.0 + 15.0 = 145.0. Tax: 9.06. Total = 154.06
    let invoice = build_base_invoice("dummy_sig".to_string(), 154.06);

    assert!(invoice.validate_amount_plus_tax_equals_total().is_ok());
    assert!(invoice.validate_totals().is_ok());
    assert!(invoice.validate_integrity().is_ok());
}

#[test]
fn test_invoice_math_validation_failure_on_imbalance() {
    let invalid_invoice = build_base_invoice("dummy_sig".to_string(), 999.99);

    let err = invalid_invoice
        .validate_amount_plus_tax_equals_total()
        .expect_err("must fail arithmetic validation");
    assert_eq!(err.field_path, "total");
    assert!(err.error_message.contains("does not equal total"));

    let integrity_errs = invalid_invoice.validate_integrity().unwrap_err();
    assert_eq!(integrity_errs.len(), 1);
    assert_eq!(integrity_errs[0].field_path, "total");
}

#[test]
fn test_signature_variant_image_section() {
    let path = fixture_invoice_path();
    assert!(path.exists(), "fixture invoice_example.png must exist");

    let dyn_img = image::open(&path).expect("open invoice image");
    let (w, h) = dyn_img.dimensions();
    assert_eq!(w, 750);
    assert_eq!(h, 1061);

    let bbox = signature_bounding_box();
    let rgba_img = dyn_img.to_rgba8();

    // Direct extraction via VisualRegion
    let visual_region = VisualRegion::extract_from_image(&rgba_img, &bbox);
    assert!(visual_region.width() > 100);
    assert!(visual_region.height() > 80);
    assert_eq!(visual_region.bbox(), &bbox);

    // Extraction via PageRasterBuffer
    let buffer = PageRasterBuffer::new(0, rgba_img, 150).unwrap();
    let sig_from_buffer = buffer.extract_region(&bbox).expect("extract visual region");
    assert_eq!(sig_from_buffer.dimensions(), visual_region.dimensions());

    // Encode to ImageCrop
    let crop = visual_region
        .to_image_crop(CropFormat::Png)
        .expect("encode signature crop");
    assert_eq!(crop.mime_type, "image/png");
    assert!(!crop.encoded_data.is_empty());
    assert!(crop.width > 0 && crop.height > 0);

    // Build invoice with SignaturePayload::Image variant
    let signature_variant = SignaturePayload::Image {
        crop: crop.clone().into(),
    };
    let invoice = build_base_invoice(signature_variant, 154.06);

    assert!(invoice.validate_amount_plus_tax_equals_total().is_ok());
    match &invoice.signature {
        SignaturePayload::Image { crop: c } => {
            assert_eq!(c.mime_type, "image/png");
            assert_eq!(c.width, crop.width);
            assert_eq!(c.height, crop.height);
        }
        _ => panic!("Expected Image variant"),
    }
}

#[test]
fn test_signature_variant_mask_only() {
    let path = fixture_invoice_path();
    let dyn_img = image::open(&path).expect("open invoice image");
    let bbox = signature_bounding_box();
    let visual_region = VisualRegion::extract_from_image(&dyn_img.to_rgba8(), &bbox);

    // Generate binary segmentation mask
    let mask = visual_region.mask_only();
    assert_eq!(mask.dimensions(), visual_region.dimensions());

    let mut white_pixels = 0u32;
    let mut black_pixels = 0u32;

    for p in mask.pixels() {
        if p[0] == 255 && p[1] == 255 && p[2] == 255 && p[3] == 255 {
            white_pixels += 1;
        } else if p[0] == 0 && p[1] == 0 && p[2] == 0 && p[3] == 255 {
            black_pixels += 1;
        } else {
            panic!("Mask must be strictly binary, found non-binary pixel: {:?}", p);
        }
    }

    assert!(white_pixels > 500, "Mask must contain white foreground signature strokes");
    assert!(black_pixels > 5000, "Mask must contain black background");

    // Also verify black-on-white mask variant
    let bow_mask = visual_region.mask_black_on_white();
    assert_eq!(bow_mask.dimensions(), visual_region.dimensions());
    let bow_black = bow_mask.pixels().filter(|p| p[0] == 0 && p[3] == 255).count();
    assert_eq!(bow_black as u32, white_pixels);

    // Encode mask to PNG base64 for SignaturePayload::Mask via visual_region helper
    let mask_region = VisualRegion::new(mask.clone(), bbox);
    let mask_crop = mask_region.to_image_crop(CropFormat::Png).unwrap();

    let signature_variant = SignaturePayload::Mask {
        mask_png_base64: mask_crop.encoded_data,
        width: mask.width(),
        height: mask.height(),
    };
    let invoice = build_base_invoice(signature_variant, 154.06);

    assert!(invoice.validate_amount_plus_tax_equals_total().is_ok());
    match &invoice.signature {
        SignaturePayload::Mask { mask_png_base64, width, height } => {
            assert!(!mask_png_base64.is_empty());
            assert_eq!(*width, mask.width());
            assert_eq!(*height, mask.height());
        }
        _ => panic!("Expected Mask variant"),
    }
}

#[test]
fn test_signature_variant_grayscale() {
    let path = fixture_invoice_path();
    let dyn_img = image::open(&path).expect("open invoice image");
    let bbox = signature_bounding_box();
    let visual_region = VisualRegion::extract_from_image(&dyn_img.to_rgba8(), &bbox);

    // Convert to grayscale keeping only grayscale luminance
    let gray_img = visual_region.grayscale();
    assert_eq!(gray_img.dimensions(), visual_region.dimensions());

    // Verify all pixels have identical R, G, B channels (pure grayscale, zero chroma)
    for p in gray_img.pixels() {
        assert_eq!(
            p[0], p[1],
            "Grayscale pixel must have R == G (found R={}, G={})",
            p[0], p[1]
        );
        assert_eq!(
            p[1], p[2],
            "Grayscale pixel must have G == B (found G={}, B={})",
            p[1], p[2]
        );
    }

    // Encode grayscale to PNG base64 for SignaturePayload::Grayscale
    let gray_region = VisualRegion::new(gray_img.clone(), bbox);
    let gray_crop = gray_region.to_image_crop(CropFormat::Png).unwrap();

    let signature_variant = SignaturePayload::Grayscale {
        grayscale_png_base64: gray_crop.encoded_data,
        width: gray_img.width(),
        height: gray_img.height(),
    };
    let invoice = build_base_invoice(signature_variant, 154.06);

    assert!(invoice.validate_amount_plus_tax_equals_total().is_ok());
    match &invoice.signature {
        SignaturePayload::Grayscale { grayscale_png_base64, width, height } => {
            assert!(!grayscale_png_base64.is_empty());
            assert_eq!(*width, gray_img.width());
            assert_eq!(*height, gray_img.height());
        }
        _ => panic!("Expected Grayscale variant"),
    }
}

#[test]
fn test_signature_variant_vectorized() {
    let path = fixture_invoice_path();
    let dyn_img = image::open(&path).expect("open invoice image");
    let bbox = signature_bounding_box();
    let visual_region = VisualRegion::extract_from_image(&dyn_img.to_rgba8(), &bbox);

    // Vectorize the signature region into SVG and polygons
    let vectorized: VectorizedRegion = visual_region.vectorize();
    assert_eq!(vectorized.width, visual_region.width());
    assert_eq!(vectorized.height, visual_region.height());
    assert_eq!(vectorized.bbox, bbox);

    // Verify SVG markup
    let svg = vectorized.to_svg();
    assert!(svg.starts_with("<svg "), "Must begin with valid <svg tag");
    assert!(svg.ends_with("</svg>"), "Must terminate with </svg>");
    assert!(svg.contains("fill=\"#000000\""), "SVG must define fill color");
    assert!(svg.contains("viewBox="), "SVG must have viewBox attribute");
    assert!(svg.contains("<path d="), "SVG must contain vector path elements");
    assert!(!vectorized.path_data().is_empty(), "Path data must be non-empty");

    // Verify vector polygons
    assert!(
        vectorized.polygon_count() >= 1,
        "Must trace at least 1 vector polygon contour for handwritten strokes (got {})",
        vectorized.polygon_count()
    );

    for poly in vectorized.polygons() {
        assert!(
            poly.vertices.len() >= 3,
            "Each polygon contour must have at least 3 vertices"
        );
        let poly_bbox = poly.bounding_box();
        assert!(poly_bbox.top_left.x >= 0.0 && poly_bbox.top_left.x <= 1.0);
        assert!(poly_bbox.top_left.y >= 0.0 && poly_bbox.top_left.y <= 1.0);
        assert!(poly_bbox.bottom_right.x >= 0.0 && poly_bbox.bottom_right.x <= 1.0);
        assert!(poly_bbox.bottom_right.y >= 0.0 && poly_bbox.bottom_right.y <= 1.0);
    }

    // Build invoice with SignaturePayload::Vectorized variant
    let signature_variant = SignaturePayload::Vectorized {
        svg: vectorized.svg.clone(),
        path_data: vectorized.path_data.clone(),
        width: vectorized.width,
        height: vectorized.height,
    };
    let invoice = build_base_invoice(signature_variant, 154.06);

    assert!(invoice.validate_amount_plus_tax_equals_total().is_ok());
    match &invoice.signature {
        SignaturePayload::Vectorized { svg: s, path_data: p, width, height } => {
            assert!(s.contains("<svg"));
            assert!(!p.is_empty());
            assert_eq!(*width, vectorized.width);
            assert_eq!(*height, vectorized.height);
        }
        _ => panic!("Expected Vectorized variant"),
    }
}

#[tokio::test]
async fn test_end_to_end_invoice_example_pipeline_extraction() {
    let path = fixture_invoice_path();
    assert!(path.exists());

    let bbox = signature_bounding_box();
    let dyn_img = image::open(&path).expect("open invoice image");
    let visual_region = VisualRegion::extract_from_image(&dyn_img.to_rgba8(), &bbox);
    let sig_crop = visual_region.to_image_crop(CropFormat::Png).unwrap();

    let expected_invoice: Invoice<SignaturePayload> = build_base_invoice(
        SignaturePayload::Image { crop: sig_crop.into() },
        154.06,
    );

    let canned_json = serde_json::to_string_pretty(&expected_invoice).unwrap();
    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));

    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let source = DocumentSource::from_image_path(path);
    let result = pipeline
        .process_document::<Invoice<SignaturePayload>>("doc-invoice-us-001", &source)
        .await
        .expect("process invoice_example.png");

    assert_eq!(result.document_id, "doc-invoice-us-001");
    assert!(!result.routed_to_triage);
    assert!(result.confidence >= 0.85);

    let extracted = result.data.expect("parsed invoice data");
    assert_eq!(extracted.merchant.name, "East Repair Inc.");
    assert_eq!(extracted.bill_info.username, "John Smith");
    assert_eq!(extracted.receiver.username, "John Smith");
    assert_eq!(extracted.items.len(), 3);
    assert_eq!(extracted.items[0].description, "Front and rear brake cables");
    assert_eq!(extracted.taxes.len(), 1);
    assert_eq!(extracted.taxes[0].name, "Sales Tax 6.25%");
    assert_eq!(extracted.taxes[0].qty, 9.06);
    assert_eq!(extracted.total, 154.06);

    // Validate arithmetic integrity
    assert!(extracted.validate_amount_plus_tax_equals_total().is_ok());

    // Validate signature variant extraction
    match extracted.signature {
        SignaturePayload::Image { crop } => {
            assert_eq!(crop.mime_type, "image/png");
            assert!(crop.width > 0 && crop.height > 0);
            assert!(!crop.encoded_data.is_empty());
        }
        _ => panic!("Expected SignaturePayload::Image variant"),
    }
}
