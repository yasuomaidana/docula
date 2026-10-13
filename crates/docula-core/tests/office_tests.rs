use docula_core::{
    DocumentSource, OfficeFormat, PresentationSubtype, SpreadsheetSubtype, WordSubtype,
};
use std::path::PathBuf;

#[test]
fn test_office_format_detection_from_extensions() {
    let xlsx = OfficeFormat::detect(Some(&PathBuf::from("ledger.xlsx")), None);
    assert_eq!(
        xlsx,
        Some(OfficeFormat::Spreadsheet(SpreadsheetSubtype::Xlsx))
    );

    let csv = OfficeFormat::detect(Some(&PathBuf::from("transactions.CSV")), None);
    assert_eq!(
        csv,
        Some(OfficeFormat::Spreadsheet(SpreadsheetSubtype::Csv))
    );

    let tsv = OfficeFormat::detect(Some(&PathBuf::from("export.tsv")), None);
    assert_eq!(
        tsv,
        Some(OfficeFormat::Spreadsheet(SpreadsheetSubtype::Tsv))
    );

    let ods = OfficeFormat::detect(Some(&PathBuf::from("table.ods")), None);
    assert_eq!(
        ods,
        Some(OfficeFormat::Spreadsheet(SpreadsheetSubtype::Ods))
    );

    let pptx = OfficeFormat::detect(Some(&PathBuf::from("slides.pptx")), None);
    assert_eq!(
        pptx,
        Some(OfficeFormat::Presentation(PresentationSubtype::Pptx))
    );

    let odp = OfficeFormat::detect(Some(&PathBuf::from("pitch.odp")), None);
    assert_eq!(
        odp,
        Some(OfficeFormat::Presentation(PresentationSubtype::Odp))
    );

    let docx = OfficeFormat::detect(Some(&PathBuf::from("contract.docx")), None);
    assert_eq!(docx, Some(OfficeFormat::WordProcessing(WordSubtype::Docx)));

    let odt = OfficeFormat::detect(Some(&PathBuf::from("notes.odt")), None);
    assert_eq!(odt, Some(OfficeFormat::WordProcessing(WordSubtype::Odt)));

    let txt = OfficeFormat::detect(Some(&PathBuf::from("memo.txt")), None);
    assert_eq!(
        txt,
        Some(OfficeFormat::WordProcessing(WordSubtype::PlainText))
    );

    let rtf = OfficeFormat::detect(Some(&PathBuf::from("letter.rtf")), None);
    assert_eq!(rtf, Some(OfficeFormat::WordProcessing(WordSubtype::Rtf)));
}

#[test]
fn test_office_format_query_helpers() {
    let xlsx = OfficeFormat::Spreadsheet(SpreadsheetSubtype::Xlsx);
    assert!(xlsx.is_spreadsheet());
    assert!(!xlsx.is_presentation());
    assert!(!xlsx.is_word_processing());
    assert_eq!(
        xlsx.mime_type(),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );

    let pptx = OfficeFormat::Presentation(PresentationSubtype::Pptx);
    assert!(!pptx.is_spreadsheet());
    assert!(pptx.is_presentation());
    assert!(!pptx.is_word_processing());

    let docx = OfficeFormat::WordProcessing(WordSubtype::Docx);
    assert!(!docx.is_spreadsheet());
    assert!(!docx.is_presentation());
    assert!(docx.is_word_processing());
}

#[test]
fn test_document_source_office_variants() {
    let bytes = b"vendor,subtotal,tax,total\nAcme,100,10,110\n".to_vec();
    let source = DocumentSource::from_office_bytes(
        bytes.clone(),
        OfficeFormat::Spreadsheet(SpreadsheetSubtype::Csv),
    );

    assert!(source.is_office());
    assert!(source.is_spreadsheet());
    assert!(!source.is_presentation());
    assert!(!source.is_pdf());
    assert!(!source.is_image());
    assert_eq!(source.byte_len(), Some(bytes.len()));
    assert_eq!(source.as_bytes(), Some(bytes.as_slice()));

    // From PathBuf auto-detection
    let path = PathBuf::from("invoice_report.xlsx");
    let from_path = DocumentSource::from(path.clone());
    assert!(from_path.is_office());
    assert!(from_path.is_spreadsheet());
    assert_eq!(
        from_path.office_format(),
        Some(OfficeFormat::Spreadsheet(SpreadsheetSubtype::Xlsx))
    );
    assert_eq!(from_path.as_path(), Some(path.as_path()));
}
