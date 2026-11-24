use docula_core::DocumentSource;
use docula_ingest::{DocumentRouter, PageRoute, RouterConfig};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};

/// Assembles in-memory PDFs page by page.
struct PdfBuilder {
    doc: Document,
    pages_id: ObjectId,
    kids: Vec<Object>,
}

impl PdfBuilder {
    fn new() -> Self {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        Self {
            doc,
            pages_id,
            kids: Vec::new(),
        }
    }

    /// Adds a `Type1` Helvetica font object.
    fn helvetica(&mut self) -> ObjectId {
        self.doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
            "Encoding" => "WinAnsiEncoding",
        })
    }

    /// Adds an uncompressed `size` x `size` RGB image XObject filled with `fill`.
    fn image(&mut self, size: i64, fill: u8) -> ObjectId {
        let bytes = vec![fill; (size * size * 3) as usize];
        self.doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => size,
                "Height" => size,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            },
            bytes,
        ))
    }

    /// Appends a page drawing `content` with the given `resources`.
    fn page(&mut self, resources: Dictionary, content: &[u8]) {
        let resources_id = self.doc.add_object(resources);
        let content_id = self
            .doc
            .add_object(Stream::new(dictionary! {}, content.to_vec()));
        let page_id = self.doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => self.pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        self.kids.push(page_id.into());
    }

    fn finish(mut self) -> Vec<u8> {
        let count = self.kids.len() as i64;
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => self.kids,
            "Count" => count,
        };
        self.doc
            .set_object(self.pages_id, Object::Dictionary(pages_dict));

        let catalog_id = self.doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => self.pages_id,
        });
        self.doc.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        self.doc.save_to(&mut bytes).expect("save pdf to bytes");
        bytes
    }
}

/// Helper to create a valid single-page born-digital PDF in memory.
fn create_born_digital_pdf(text: &str) -> Vec<u8> {
    let mut pdf = PdfBuilder::new();
    let font_id = pdf.helvetica();
    let content = format!("BT /F1 12 Tf 72 712 Td ({text}) Tj ET");
    pdf.page(
        dictionary! { "Font" => dictionary! { "F1" => font_id } },
        content.as_bytes(),
    );
    pdf.finish()
}

/// Helper to create a scanned PDF (Image XObject only, zero text layer).
fn create_scanned_image_pdf() -> Vec<u8> {
    let mut pdf = PdfBuilder::new();
    let image_id = pdf.image(200, 200);
    pdf.page(
        dictionary! { "XObject" => dictionary! { "Im1" => image_id } },
        b"q 200 0 0 200 50 50 cm /Im1 Do Q",
    );
    pdf.finish()
}

/// Helper to create a PDF with a corrupt `/ToUnicode` stream.
fn create_corrupt_tounicode_pdf() -> Vec<u8> {
    let mut pdf = PdfBuilder::new();

    // Stream marked as ToUnicode, but containing broken garbage with no valid CMap markers
    let corrupt_stream_id = pdf.doc.add_object(Stream::new(
        dictionary! {},
        b"BROKEN_CORRUPTED_STREAM_GARBAGE_NO_CMAP".to_vec(),
    ));
    let font_id = pdf.doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "CorruptedFont-Identity-H",
        "Encoding" => "Identity-H",
        "ToUnicode" => corrupt_stream_id,
    });
    pdf.page(
        dictionary! { "Font" => dictionary! { "F1" => font_id } },
        b"BT /F1 12 Tf 50 700 Td (Unreadable text due to broken font) Tj ET",
    );
    pdf.finish()
}

/// Helper to create a 2-page hybrid PDF (page 0 digital, page 1 scanned).
fn create_hybrid_pdf() -> Vec<u8> {
    let mut pdf = PdfBuilder::new();

    // Page 1: Digital
    let font_id = pdf.helvetica();
    pdf.page(
        dictionary! { "Font" => dictionary! { "F1" => font_id } },
        b"BT /F1 12 Tf 72 700 Td (Page One Born-Digital Invoice Total $1,250.00) Tj ET",
    );

    // Page 2: Scanned Image
    let image_id = pdf.image(100, 128);
    pdf.page(
        dictionary! { "XObject" => dictionary! { "Im1" => image_id } },
        b"q 100 0 0 100 50 50 cm /Im1 Do Q",
    );
    pdf.finish()
}

#[test]
fn test_route_born_digital_pdf() {
    let pdf_bytes =
        create_born_digital_pdf("Tax Invoice #10423 Subtotal: $450.00 Tax: $45.00 Total: $495.00");
    let router = DocumentRouter::new();

    let report = router
        .route_pdf_bytes(&pdf_bytes)
        .expect("routing succeeded");
    assert_eq!(report.total_pages, 1);
    assert_eq!(report.born_digital_count(), 1);
    assert_eq!(report.needs_ocr_count(), 0);
    assert!(report.is_entirely_digital());

    match &report.routes[0] {
        PageRoute::BornDigital {
            page_index,
            text_layer,
        } => {
            assert_eq!(*page_index, 0);
            assert!(
                !text_layer.is_empty(),
                "text layer should contain spatial spans"
            );
            let combined_text: String = text_layer
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            assert!(combined_text.contains("Tax Invoice #10423"));
        }
        PageRoute::NeedsOcr { .. } => panic!("Expected BornDigital route, got NeedsOcr"),
    }
}

#[test]
fn test_route_scanned_receipt_pdf() {
    let pdf_bytes = create_scanned_image_pdf();
    let router = DocumentRouter::new();

    let report = router
        .route_pdf_bytes(&pdf_bytes)
        .expect("routing succeeded");
    assert_eq!(report.total_pages, 1);
    assert_eq!(report.born_digital_count(), 0);
    assert_eq!(report.needs_ocr_count(), 1);
    assert!(report.requires_ocr());

    match &report.routes[0] {
        PageRoute::NeedsOcr {
            page_index,
            raster_job,
        } => {
            assert_eq!(*page_index, 0);
            assert_eq!(raster_job.page_index, 0);
            assert_eq!(raster_job.dpi, 300);
        }
        PageRoute::BornDigital { .. } => {
            panic!("Expected NeedsOcr for scanned PDF, got BornDigital")
        }
    }
}

#[test]
fn test_route_corrupt_tounicode() {
    let pdf_bytes = create_corrupt_tounicode_pdf();
    let router = DocumentRouter::new();

    let report = router
        .route_pdf_bytes(&pdf_bytes)
        .expect("routing succeeded");
    assert_eq!(report.total_pages, 1);
    // Because ToUnicode is corrupted, text extraction cannot reliably decipher glyphs -> must route to OCR
    assert_eq!(report.needs_ocr_count(), 1);
    assert_eq!(report.born_digital_count(), 0);

    match &report.routes[0] {
        PageRoute::NeedsOcr {
            page_index,
            raster_job,
        } => {
            assert_eq!(*page_index, 0);
            assert_eq!(raster_job.dpi, 300);
        }
        PageRoute::BornDigital { .. } => panic!("Corrupted ToUnicode must route to NeedsOcr"),
    }
}

#[test]
fn test_route_image_document_source() {
    let router = DocumentRouter::with_config(RouterConfig::default().with_default_dpi(150));
    let source = DocumentSource::from_image_bytes(vec![1, 2, 3, 4], Some("png"));

    let report = router.route_source(&source).expect("route image source");
    assert_eq!(report.total_pages, 1);
    assert_eq!(report.needs_ocr_count(), 1);
    match &report.routes[0] {
        PageRoute::NeedsOcr {
            page_index,
            raster_job,
        } => {
            assert_eq!(*page_index, 0);
            assert_eq!(raster_job.dpi, 150);
        }
        _ => panic!("Expected NeedsOcr for raw image source"),
    }
}

#[test]
fn test_route_empty_payload_fails() {
    let router = DocumentRouter::new();
    let err = router.route_pdf_bytes(&[]).unwrap_err();
    assert_eq!(err.to_string(), "Document payload is empty");
}

#[test]
fn test_route_multi_page_hybrid() {
    let pdf_bytes = create_hybrid_pdf();
    let router = DocumentRouter::new();

    let report = router
        .route_pdf_bytes(&pdf_bytes)
        .expect("route hybrid pdf");
    assert_eq!(report.total_pages, 2);
    assert_eq!(report.born_digital_count(), 1);
    assert_eq!(report.needs_ocr_count(), 1);
    assert!(report.requires_ocr());
    assert!(!report.is_entirely_digital());

    assert!(report.routes[0].is_born_digital());
    assert!(report.routes[1].needs_ocr());
}
