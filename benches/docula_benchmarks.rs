use criterion::{Criterion, black_box, criterion_group, criterion_main};
use docula::compliance::ComplianceRedactor;
use docula::exemplar::compute_layout_fingerprint;
use docula::ocr::SpatialLayoutMap;
use docula::schema::{DfaLogitsMasker, SchemaDfa};
use docula::vision_codes::OpticalCodeDetector;
use docula::{BoundingBox, SpatialSpan};
use image::{Rgba, RgbaImage};
use rxing::{BarcodeFormat, MultiFormatWriter, Writer};

fn generate_synthetic_spans(count: usize) -> Vec<SpatialSpan> {
    let mut spans = Vec::with_capacity(count);
    for i in 0..count {
        let row = (i / 5) as f32;
        let col = (i % 5) as f32;
        let y = (0.05 + row * 0.08).min(0.95);
        let x = (0.05 + col * 0.18).min(0.95);
        let bbox = BoundingBox::from_coords_clamped(x, y, (x + 0.15).min(1.0), (y + 0.04).min(1.0));
        let span = SpatialSpan::new(format!("Span_{i}"), bbox, 0, 0.98).unwrap();
        spans.push(span);
    }
    spans
}

fn bench_dfa(c: &mut Criterion) {
    let schema_json = r#"{
        "type": "object",
        "properties": {
            "invoice_number": { "type": "string" },
            "total": { "type": "number" },
            "customer": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "tax_id": { "type": "string" }
                }
            }
        }
    }"#;

    c.bench_function("dfa_compile_json", |b| {
        b.iter(|| {
            let _ = SchemaDfa::compile_json(black_box(schema_json)).unwrap();
        });
    });

    let dfa = SchemaDfa::compile_json(schema_json).unwrap();
    let masker = DfaLogitsMasker::new(dfa);
    let vocab = vec![
        "{\n".to_string(),
        "  \"invoice_number\": \"INV-123\",\n".to_string(),
        "  \"total\": 100.0\n".to_string(),
        "}".to_string(),
        "invalid_token\n".to_string(),
    ];
    let prefix = "{\n  \"invoice_number\": \"INV-123\",\n";

    c.bench_function("logits_masking_step", |b| {
        b.iter(|| {
            let mut logits = vec![1.0; vocab.len()];
            masker.mask_logits(black_box(prefix), &mut logits, black_box(&vocab));
        });
    });
}

fn bench_spatial_layout_and_fingerprint(c: &mut Criterion) {
    let spans = generate_synthetic_spans(50);
    let layout = SpatialLayoutMap::from_spans(spans.clone(), (1000, 1000));

    c.bench_function("spatial_layout_clustering_50_spans", |b| {
        b.iter(|| {
            let _ = SpatialLayoutMap::from_spans(black_box(spans.clone()), (1000, 1000));
        });
    });

    c.bench_function("simhash_fingerprinting_50_spans", |b| {
        b.iter(|| {
            let _ = compute_layout_fingerprint(black_box(&layout));
        });
    });
}

fn bench_compliance_and_vision_codes(c: &mut Criterion) {
    let redactor = ComplianceRedactor::default();
    let spans = vec![
        SpatialSpan::new(
            "IBAN: ES1234567890123456789012",
            BoundingBox::from_coords_clamped(0.1, 0.1, 0.4, 0.15),
            0,
            0.99,
        )
        .unwrap(),
        SpatialSpan::new(
            "Invoice Total: $120.00",
            BoundingBox::from_coords_clamped(0.1, 0.2, 0.3, 0.25),
            0,
            0.99,
        )
        .unwrap(),
    ];
    let img = RgbaImage::new(400, 400);

    c.bench_function("compliance_auto_censor", |b| {
        b.iter(|| {
            let mut test_img = img.clone();
            let mut test_spans = spans.clone();
            let _ = redactor.auto_censor(&mut test_img, &mut test_spans);
        });
    });

    let bitmatrix = MultiFormatWriter
        .encode(
            "https://example.com/inv/123",
            &BarcodeFormat::QR_CODE,
            150,
            150,
        )
        .expect("QR encode");
    let mut qr_img = RgbaImage::new(150, 150);
    for y in 0..150 {
        for x in 0..150 {
            let pixel = if bitmatrix.get(x, y) {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };
            qr_img.put_pixel(x, y, pixel);
        }
    }
    let detector = OpticalCodeDetector::new();

    c.bench_function("optical_qr_detect", |b| {
        b.iter(|| {
            let _ = detector.detect(black_box(&qr_img)).unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_dfa,
    bench_spatial_layout_and_fingerprint,
    bench_compliance_and_vision_codes
);
criterion_main!(benches);
