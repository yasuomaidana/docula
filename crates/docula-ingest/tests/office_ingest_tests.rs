use docula_core::{DocumentSource, OfficeFormat, PresentationSubtype, SpreadsheetSubtype};
use docula_ingest::{
    DocumentRouter, PageRoute, PresentationReader, SpreadsheetReader, WordProcessingReader,
};
use std::io::Write;
use zip::write::SimpleFileOptions;

/// Helper to construct a minimal valid in-memory XLSX ZIP archive.
fn create_mock_xlsx_bytes() -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        // 1. Shared strings
        zip.start_file("xl/sharedStrings.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3" uniqueCount="3">
    <si><t>Invoice Number</t></si>
    <si><t>INV-2026-9901</t></si>
    <si><t>Total Amount</t></si>
</sst>"#,
        )
        .unwrap();

        // 2. Worksheet 1
        zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
    <sheetData>
        <row r="1">
            <c r="A1" t="s"><v>0</v></c>
            <c r="B1" t="s"><v>1</v></c>
        </row>
        <row r="2">
            <c r="A2" t="s"><v>2</v></c>
            <c r="B2"><v>1499.50</v></c>
        </row>
    </sheetData>
</worksheet>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }
    buf
}

/// Helper to construct a minimal valid in-memory PPTX ZIP archive with 2 slides.
fn create_mock_pptx_bytes() -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        // Slide 1
        zip.start_file("ppt/slides/slide1.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
    <p:cSld>
        <p:spTree>
            <p:sp>
                <p:txBody>
                    <a:p><a:r><a:t>Q3 Enterprise Earnings Call</a:t></a:r></a:p>
                    <a:p><a:r><a:t>Executive Summary</a:t></a:r></a:p>
                </p:txBody>
            </p:sp>
        </p:spTree>
    </p:cSld>
</p:sld>"#,
        )
        .unwrap();

        // Slide 2
        zip.start_file("ppt/slides/slide2.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
    <p:cSld>
        <p:spTree>
            <p:sp>
                <p:txBody>
                    <a:p><a:r><a:t>Financial Highlights: Net Revenue $4.2M</a:t></a:r></a:p>
                    <a:p><a:r><a:t>Operating Margin: 32%</a:t></a:r></a:p>
                </p:txBody>
            </p:sp>
        </p:spTree>
    </p:cSld>
</p:sld>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }
    buf
}

/// Helper to construct a minimal valid in-memory DOCX ZIP archive.
fn create_mock_docx_bytes() -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
    <w:body>
        <w:p><w:r><w:t>Master Service Agreement</w:t></w:r></w:p>
        <w:p><w:r><w:t>Between Acme Corporation and Global Logistics LLC</w:t></w:r></w:p>
        <w:p><w:r><w:t>Payment terms: Net 30 days upon receipt of invoice.</w:t></w:r></w:p>
    </w:body>
</w:document>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }
    buf
}

#[test]
fn test_csv_and_tsv_spreadsheet_ingestion() {
    let csv_data = b"Vendor,Amount,Status\nAcme Corp,1200.50,Paid\nBeta LLC,850.00,Pending\n";
    let workbook = SpreadsheetReader::read_csv(csv_data).expect("parses csv successfully");

    assert_eq!(workbook.sheets.len(), 1);
    let sheet = &workbook.sheets[0];
    assert_eq!(sheet.cells.len(), 9);
    assert_eq!(sheet.cells[0].value, "Vendor");
    assert_eq!(sheet.cells[0].reference, "A1");
    assert_eq!(sheet.cells[1].value, "Amount");
    assert_eq!(sheet.cells[1].reference, "B1");

    let spans = sheet.to_spatial_spans();
    assert_eq!(spans.len(), 9);
    assert_eq!(spans[0].text, "Vendor");
    assert_eq!(spans[0].confidence, 1.0);

    // Test TSV
    let tsv_data = b"Product\tPrice\nKeyboard\t79.99\nMouse\t29.99\n";
    let tsv_wb = SpreadsheetReader::read_tsv(tsv_data).expect("parses tsv");
    assert_eq!(tsv_wb.sheets[0].cells.len(), 6);
    assert_eq!(tsv_wb.sheets[0].cells[2].value, "Keyboard");
}

#[test]
fn test_real_xlsx_spreadsheet_ingestion() {
    let xlsx_bytes = create_mock_xlsx_bytes();
    let workbook = SpreadsheetReader::read_xlsx(&xlsx_bytes).expect("parses xlsx");

    assert_eq!(workbook.sheets.len(), 1);
    let sheet = &workbook.sheets[0];
    assert_eq!(sheet.cells.len(), 4);

    // Shared strings resolved accurately:
    assert_eq!(sheet.cells[0].reference, "A1");
    assert_eq!(sheet.cells[0].value, "Invoice Number");
    assert_eq!(sheet.cells[1].reference, "B1");
    assert_eq!(sheet.cells[1].value, "INV-2026-9901");
    assert_eq!(sheet.cells[2].reference, "A2");
    assert_eq!(sheet.cells[2].value, "Total Amount");
    assert_eq!(sheet.cells[3].reference, "B2");
    assert_eq!(sheet.cells[3].value, "1499.50");

    let spans = sheet.to_spatial_spans();
    assert_eq!(spans.len(), 4);
    assert_eq!(spans[1].text, "INV-2026-9901");
}

#[test]
fn test_real_pptx_presentation_ingestion() {
    let pptx_bytes = create_mock_pptx_bytes();
    let deck = PresentationReader::read_pptx(&pptx_bytes).expect("parses pptx");

    assert_eq!(deck.slides.len(), 2);

    // Slide 1
    assert_eq!(deck.slides[0].slide_index, 0);
    assert_eq!(deck.slides[0].spans.len(), 2);
    assert_eq!(deck.slides[0].spans[0].text, "Q3 Enterprise Earnings Call");
    assert_eq!(deck.slides[0].spans[1].text, "Executive Summary");

    // Slide 2
    assert_eq!(deck.slides[1].slide_index, 1);
    assert_eq!(deck.slides[1].spans.len(), 2);
    assert_eq!(
        deck.slides[1].spans[0].text,
        "Financial Highlights: Net Revenue $4.2M"
    );

    let routes = deck.to_page_routes();
    assert_eq!(routes.len(), 2);
    assert!(routes[0].is_born_digital());
    assert!(routes[1].is_born_digital());
}

#[test]
fn test_real_docx_and_text_ingestion() {
    let docx_bytes = create_mock_docx_bytes();
    let doc = WordProcessingReader::read_docx(&docx_bytes).expect("parses docx");

    assert_eq!(doc.pages.len(), 1);
    assert_eq!(doc.pages[0].len(), 3);
    assert_eq!(doc.pages[0][0].text, "Master Service Agreement");
    assert_eq!(
        doc.pages[0][1].text,
        "Between Acme Corporation and Global Logistics LLC"
    );

    // Plain text test
    let raw_txt = b"Header: System Status Report\nTimestamp: 2026-10-13\nAll nodes operational.\n";
    let txt_doc = WordProcessingReader::read_plain_text(raw_txt).expect("parses txt");
    assert_eq!(txt_doc.pages[0].len(), 3);
    assert_eq!(txt_doc.pages[0][0].text, "Header: System Status Report");
}

#[test]
fn test_document_router_office_integration() {
    let router = DocumentRouter::new();

    // 1. XLSX through DocumentRouter
    let xlsx_bytes = create_mock_xlsx_bytes();
    let source_xlsx = DocumentSource::from_office_bytes(
        xlsx_bytes,
        OfficeFormat::Spreadsheet(SpreadsheetSubtype::Xlsx),
    );
    let report_xlsx = router
        .route_source(&source_xlsx)
        .expect("router processes xlsx");

    assert_eq!(report_xlsx.total_pages, 1);
    assert!(report_xlsx.is_entirely_digital());
    assert!(!report_xlsx.requires_ocr());
    if let PageRoute::BornDigital { text_layer, .. } = &report_xlsx.routes[0] {
        assert_eq!(text_layer.len(), 4);
        assert_eq!(text_layer[0].text, "Invoice Number");
    } else {
        panic!("Expected born digital route");
    }

    // 2. PPTX through DocumentRouter
    let pptx_bytes = create_mock_pptx_bytes();
    let source_pptx = DocumentSource::from_office_bytes(
        pptx_bytes,
        OfficeFormat::Presentation(PresentationSubtype::Pptx),
    );
    let report_pptx = router
        .route_source(&source_pptx)
        .expect("router processes pptx");

    assert_eq!(report_pptx.total_pages, 2);
    assert!(report_pptx.is_entirely_digital());
    assert!(!report_pptx.requires_ocr());
}
