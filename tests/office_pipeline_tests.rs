use docula::{
    DoculaPipeline, DocumentSource, ExtractionResult, OfficeFormat, PresentationSubtype,
    SpreadsheetSubtype, WordSubtype, docula_model,
};
use docula_llm::MockLlmBackend;
use std::io::Write;
use std::sync::Arc;
use zip::write::SimpleFileOptions;

docula_model! {
    pub struct OfficeInvoice {
        pub invoice_number: String,
        pub subtotal: f64,
        pub tax: f64,
        pub total: f64,
    }
    validate(self, v) {
        v.assert_non_empty("invoice_number", &self.invoice_number, "Invoice number");
        v.assert(self.subtotal > 0.0, "subtotal", "Subtotal must be positive");
        v.assert(
            (self.total - (self.subtotal + self.tax)).abs() <= 0.05,
            "total",
            "Total must equal subtotal + tax",
        );
    }
}

docula_model! {
    pub struct PresentationSummary {
        pub title: String,
        pub revenue_millions: f64,
        pub margin_percent: f64,
    }
    validate(self, v) {
        v.assert_non_empty("title", &self.title, "Presentation title");
        v.assert(self.revenue_millions > 0.0, "revenue_millions", "Revenue must be positive");
    }
}

docula_model! {
    pub struct ContractDocument {
        pub title: String,
        pub party_a: String,
        pub party_b: String,
        pub net_days: u32,
    }
    validate(self, v) {
        v.assert_non_empty("title", &self.title, "Contract title");
        v.assert_non_empty("party_a", &self.party_a, "Party A");
        v.assert_non_empty("party_b", &self.party_b, "Party B");
    }
}

/// Helper constructing valid XLSX bytes with invoice table.
fn create_mock_xlsx_invoice() -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("xl/sharedStrings.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="4" uniqueCount="4">
    <si><t>INV-2026-XLSX-01</t></si>
    <si><t>Subtotal</t></si>
    <si><t>Tax</t></si>
    <si><t>Total</t></si>
</sst>"#,
        )
        .unwrap();

        zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
    <sheetData>
        <row r="1">
            <c r="A1" t="s"><v>0</v></c>
        </row>
        <row r="2">
            <c r="A2" t="s"><v>1</v></c>
            <c r="B2"><v>1000.00</v></c>
        </row>
        <row r="3">
            <c r="A3" t="s"><v>2</v></c>
            <c r="B3"><v>100.00</v></c>
        </row>
        <row r="4">
            <c r="A4" t="s"><v>3</v></c>
            <c r="B4"><v>1100.00</v></c>
        </row>
    </sheetData>
</worksheet>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }
    buf
}

/// Helper constructing valid PPTX bytes with executive slides.
fn create_mock_pptx_deck() -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("ppt/slides/slide1.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
    <p:cSld>
        <p:spTree>
            <p:sp>
                <p:txBody>
                    <a:p><a:r><a:t>Global Q3 Performance</a:t></a:r></a:p>
                    <a:p><a:r><a:t>Revenue $5.5M</a:t></a:r></a:p>
                    <a:p><a:r><a:t>Operating Margin 25.0%</a:t></a:r></a:p>
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

/// Helper constructing valid DOCX bytes with contract clauses.
fn create_mock_docx_contract() -> Vec<u8> {
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
        <w:p><w:r><w:t>Service Level Agreement 2026</w:t></w:r></w:p>
        <w:p><w:r><w:t>Party A: Acme Technologies Inc</w:t></w:r></w:p>
        <w:p><w:r><w:t>Party B: Apex Logistics International</w:t></w:r></w:p>
        <w:p><w:r><w:t>Net Days: 45</w:t></w:r></w:p>
    </w:body>
</w:document>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }
    buf
}

#[tokio::test]
async fn test_pipeline_xlsx_end_to_end() {
    let xlsx_bytes = create_mock_xlsx_invoice();
    let source = DocumentSource::from_office_bytes(
        xlsx_bytes,
        OfficeFormat::Spreadsheet(SpreadsheetSubtype::Xlsx),
    );

    let canned_json = serde_json::json!({
        "invoice_number": "INV-2026-XLSX-01",
        "subtotal": 1000.0,
        "tax": 100.0,
        "total": 1100.0
    })
    .to_string();

    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let res: ExtractionResult<OfficeInvoice> = pipeline
        .process_document("doc-xlsx-inv-01", &source)
        .await
        .expect("pipeline extracts xlsx successfully");

    assert!(res.outcome.is_strict());
    assert!(res.data.is_some());
    let inv = res.data.unwrap();
    assert_eq!(inv.invoice_number, "INV-2026-XLSX-01");
    assert_eq!(inv.subtotal, 1000.0);
    assert_eq!(inv.total, 1100.0);
    assert!(!res.routed_to_triage);
}

#[tokio::test]
async fn test_pipeline_pptx_end_to_end() {
    let pptx_bytes = create_mock_pptx_deck();
    let source = DocumentSource::from_office_bytes(
        pptx_bytes,
        OfficeFormat::Presentation(PresentationSubtype::Pptx),
    );

    let canned_json = serde_json::json!({
        "title": "Global Q3 Performance",
        "revenue_millions": 5.5,
        "margin_percent": 25.0
    })
    .to_string();

    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let res: ExtractionResult<PresentationSummary> = pipeline
        .process_document("doc-pptx-perf-01", &source)
        .await
        .expect("pipeline extracts pptx successfully");

    assert!(res.outcome.is_strict());
    assert!(res.data.is_some());
    let pres = res.data.unwrap();
    assert_eq!(pres.title, "Global Q3 Performance");
    assert_eq!(pres.revenue_millions, 5.5);
    assert_eq!(pres.margin_percent, 25.0);
}

#[tokio::test]
async fn test_pipeline_docx_end_to_end() {
    let docx_bytes = create_mock_docx_contract();
    let source = DocumentSource::from_office_bytes(
        docx_bytes,
        OfficeFormat::WordProcessing(WordSubtype::Docx),
    );

    let canned_json = serde_json::json!({
        "title": "Service Level Agreement 2026",
        "party_a": "Acme Technologies Inc",
        "party_b": "Apex Logistics International",
        "net_days": 45
    })
    .to_string();

    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let res: ExtractionResult<ContractDocument> = pipeline
        .process_document("doc-docx-contract-01", &source)
        .await
        .expect("pipeline extracts docx successfully");

    assert!(res.outcome.is_strict());
    assert!(res.data.is_some());
    let contract = res.data.unwrap();
    assert_eq!(contract.title, "Service Level Agreement 2026");
    assert_eq!(contract.party_a, "Acme Technologies Inc");
    assert_eq!(contract.party_b, "Apex Logistics International");
    assert_eq!(contract.net_days, 45);
}

#[tokio::test]
async fn test_pipeline_csv_spreadsheet_end_to_end() {
    let csv_bytes = b"Invoice,Subtotal,Tax,Total\nINV-CSV-88,500.0,50.0,550.0\n".to_vec();
    let source = DocumentSource::from_office_bytes(
        csv_bytes,
        OfficeFormat::Spreadsheet(SpreadsheetSubtype::Csv),
    );

    let canned_json = serde_json::json!({
        "invoice_number": "INV-CSV-88",
        "subtotal": 500.0,
        "tax": 50.0,
        "total": 550.0
    })
    .to_string();

    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let res: ExtractionResult<OfficeInvoice> = pipeline
        .process_document("doc-csv-inv-88", &source)
        .await
        .expect("pipeline extracts csv successfully");

    assert!(res.outcome.is_strict());
    assert!(res.data.is_some());
    let inv = res.data.unwrap();
    assert_eq!(inv.invoice_number, "INV-CSV-88");
    assert_eq!(inv.total, 550.0);
}
