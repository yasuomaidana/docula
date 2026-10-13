use docula_core::DocumentSource;
use docula_ingest::{DocumentRouter, PageRoute, RouterConfig};
use lopdf::{Document, Object, Stream, dictionary};

/// Helper to create a valid single-page born-digital PDF in memory.
fn create_born_digital_pdf(text: &str) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });

    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_id,
        },
    });

    let content_stream = format!("BT /F1 12 Tf 72 712 Td ({text}) Tj ET");
    let content_id = doc.add_object(Stream::new(dictionary! {}, content_stream.into_bytes()));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![page_id.into()],
        "Count" => 1,
    };
    doc.set_object(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("save pdf to bytes");
    bytes
}

/// Helper to create a scanned PDF (Image XObject only, zero text layer).
fn create_scanned_image_pdf() -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let image_xobject_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 200,
            "Height" => 200,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
        },
        vec![200u8; 120000],
    ));

    let resources_id = doc.add_object(dictionary! {
        "XObject" => dictionary! {
            "Im1" => image_xobject_id,
        },
    });

    let content_stream = b"q 200 0 0 200 50 50 cm /Im1 Do Q".to_vec();
    let content_id = doc.add_object(Stream::new(dictionary! {}, content_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![page_id.into()],
        "Count" => 1,
    };
    doc.set_object(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("save scanned pdf");
    bytes
}

/// Helper to create a PDF with a corrupt `/ToUnicode` stream.
fn create_corrupt_tounicode_pdf() -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    // Stream marked as ToUnicode, but containing broken garbage with no valid CMap markers
    let corrupt_stream_id = doc.add_object(Stream::new(
        dictionary! {},
        b"BROKEN_CORRUPTED_STREAM_GARBAGE_NO_CMAP".to_vec(),
    ));

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "CorruptedFont-Identity-H",
        "Encoding" => "Identity-H",
        "ToUnicode" => corrupt_stream_id,
    });

    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_id,
        },
    });

    let content_stream =
        b"BT /F1 12 Tf 50 700 Td (Unreadable text due to broken font) Tj ET".to_vec();
    let content_id = doc.add_object(Stream::new(dictionary! {}, content_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![page_id.into()],
        "Count" => 1,
    };
    doc.set_object(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("save corrupt tounicode pdf");
    bytes
}

/// Helper to create a 2-page hybrid PDF (page 0 digital, page 1 scanned).
fn create_hybrid_pdf() -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    // Page 1: Digital
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let res1_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });
    let c1_id = doc.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 72 700 Td (Page One Born-Digital Invoice Total $1,250.00) Tj ET".to_vec(),
    ));
    let p1_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => c1_id,
        "Resources" => res1_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });

    // Page 2: Scanned Image
    let img_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 100,
            "Height" => 100,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
        },
        vec![128u8; 30000],
    ));
    let res2_id = doc.add_object(dictionary! {
        "XObject" => dictionary! { "Im1" => img_id },
    });
    let c2_id = doc.add_object(Stream::new(
        dictionary! {},
        b"q 100 0 0 100 50 50 cm /Im1 Do Q".to_vec(),
    ));
    let p2_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => c2_id,
        "Resources" => res2_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![p1_id.into(), p2_id.into()],
        "Count" => 2,
    };
    doc.set_object(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("save hybrid pdf");
    bytes
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
