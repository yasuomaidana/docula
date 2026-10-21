use docula_core::{BoundingBox, SpatialSpan};
use docula_ocr::SpatialLayoutMap;

#[test]
fn test_multiline_invoice_header() {
    let bbox1 = BoundingBox::from_coords_clamped(0.05, 0.10, 0.40, 0.13);
    let span1 = SpatialSpan::new("INVOICE #INV-2026-9941".to_string(), bbox1, 0, 1.0).unwrap();

    let bbox2 = BoundingBox::from_coords_clamped(0.05, 0.16, 0.50, 0.19);
    let span2 = SpatialSpan::new(
        "Vendor: Acme Industrial Solutions".to_string(),
        bbox2,
        0,
        1.0,
    )
    .unwrap();

    let bbox3 = BoundingBox::from_coords_clamped(0.05, 0.22, 0.30, 0.25);
    let span3 = SpatialSpan::new("Date: 2026-10-12".to_string(), bbox3, 0, 1.0).unwrap();

    // Pass spans in reverse order
    let spans = vec![span3, span1, span2];
    let layout = SpatialLayoutMap::from_spans(spans, (1000, 1000));

    assert_eq!(layout.line_count(), 3);
    assert_eq!(layout.lines[0].text, "INVOICE #INV-2026-9941");
    assert_eq!(layout.lines[1].text, "Vendor: Acme Industrial Solutions");
    assert_eq!(layout.lines[2].text, "Date: 2026-10-12");

    let text = layout.to_plain_text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "INVOICE #INV-2026-9941");
    assert_eq!(lines[1], "Vendor: Acme Industrial Solutions");
    assert_eq!(lines[2], "Date: 2026-10-12");
}

#[test]
fn test_table_reading_order() {
    // 3 rows x 3 columns table
    // Row 0: y in [0.40, 0.43]
    let r0_c0 = SpatialSpan::new(
        "Servo Motor".to_string(),
        BoundingBox::from_coords_clamped(0.1, 0.40, 0.3, 0.43),
        0,
        1.0,
    )
    .unwrap();
    let r0_c1 = SpatialSpan::new(
        "Qty 2".to_string(),
        BoundingBox::from_coords_clamped(0.4, 0.40, 0.5, 0.43),
        0,
        1.0,
    )
    .unwrap();
    let r0_c2 = SpatialSpan::new(
        "$900.00".to_string(),
        BoundingBox::from_coords_clamped(0.6, 0.40, 0.8, 0.43),
        0,
        1.0,
    )
    .unwrap();

    // Row 1: y in [0.46, 0.49]
    let r1_c0 = SpatialSpan::new(
        "Power Supply".to_string(),
        BoundingBox::from_coords_clamped(0.1, 0.46, 0.3, 0.49),
        0,
        1.0,
    )
    .unwrap();
    let r1_c1 = SpatialSpan::new(
        "Qty 1".to_string(),
        BoundingBox::from_coords_clamped(0.4, 0.46, 0.5, 0.49),
        0,
        1.0,
    )
    .unwrap();
    let r1_c2 = SpatialSpan::new(
        "$150.00".to_string(),
        BoundingBox::from_coords_clamped(0.6, 0.46, 0.8, 0.49),
        0,
        1.0,
    )
    .unwrap();

    // Row 2: y in [0.52, 0.55]
    let r2_c0 = SpatialSpan::new(
        "Cable Harness".to_string(),
        BoundingBox::from_coords_clamped(0.1, 0.52, 0.3, 0.55),
        0,
        1.0,
    )
    .unwrap();
    let r2_c1 = SpatialSpan::new(
        "Qty 4".to_string(),
        BoundingBox::from_coords_clamped(0.4, 0.52, 0.5, 0.55),
        0,
        1.0,
    )
    .unwrap();
    let r2_c2 = SpatialSpan::new(
        "$240.00".to_string(),
        BoundingBox::from_coords_clamped(0.6, 0.52, 0.8, 0.55),
        0,
        1.0,
    )
    .unwrap();

    // Scramble input: pass column-by-column (column 0, then column 2, then column 1)
    let scrambled_spans = vec![
        r0_c0, r1_c0, r2_c0, r0_c2, r1_c2, r2_c2, r0_c1, r1_c1, r2_c1,
    ];

    let layout = SpatialLayoutMap::from_spans(scrambled_spans, (1000, 1000));
    assert_eq!(
        layout.line_count(),
        3,
        "should cluster into 3 distinct table rows"
    );

    // Verify row-by-row horizontal reading order
    assert!(layout.lines[0].text.contains("Servo Motor"));
    assert!(layout.lines[0].text.contains("Qty 2"));
    assert!(layout.lines[0].text.contains("$900.00"));

    assert!(layout.lines[1].text.contains("Power Supply"));
    assert!(layout.lines[1].text.contains("Qty 1"));
    assert!(layout.lines[1].text.contains("$150.00"));

    assert!(layout.lines[2].text.contains("Cable Harness"));
    assert!(layout.lines[2].text.contains("Qty 4"));
    assert!(layout.lines[2].text.contains("$240.00"));
}

#[test]
fn test_coordinate_formatted_text() {
    let bbox = BoundingBox::from_coords_clamped(0.05, 0.10, 0.30, 0.13);
    let span = SpatialSpan::new("Total Due: $1,250.00".to_string(), bbox, 0, 1.0).unwrap();

    let layout = SpatialLayoutMap::from_spans(vec![span], (1000, 1000));
    let formatted = layout.to_coordinate_formatted_text();

    assert!(formatted.contains("[0100, 0050] Total Due: $1,250.00"));
}

#[test]
fn test_empty_spans_handled_gracefully() {
    let layout = SpatialLayoutMap::from_spans(Vec::new(), (800, 600));
    assert_eq!(layout.line_count(), 0);
    assert!(layout.is_empty());
    assert_eq!(layout.to_plain_text(), "");
}
