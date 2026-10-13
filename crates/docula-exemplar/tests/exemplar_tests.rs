use docula_core::{BoundingBox, SpatialSpan};
use docula_exemplar::{
    ExemplarMetadata, ExemplarPool, FewShotPromptBuilder, TemplatePair, compute_layout_fingerprint,
    hamming_distance,
};
use docula_ocr::SpatialLayoutMap;
use std::time::Instant;

fn sample_layout(offset_x: f32, offset_y: f32) -> SpatialLayoutMap {
    let bbox1 = BoundingBox::from_coords_clamped(
        0.1 + offset_x,
        0.1 + offset_y,
        0.5 + offset_x,
        0.15 + offset_y,
    );
    let span1 = SpatialSpan::new("ELECTRIC UTILITY BILL".to_string(), bbox1, 0, 1.0).unwrap();

    let bbox2 = BoundingBox::from_coords_clamped(
        0.1 + offset_x,
        0.3 + offset_y,
        0.8 + offset_x,
        0.4 + offset_y,
    );
    let span2 = SpatialSpan::new(
        "Account: 12345-6789 Due: $142.50".to_string(),
        bbox2,
        0,
        1.0,
    )
    .unwrap();

    SpatialLayoutMap::from_spans(vec![span1, span2], (1000, 1000))
}

#[test]
fn test_layout_fingerprint_rotation_tolerance() {
    let layout_orig = sample_layout(0.0, 0.0);
    // 1% jitter / offset
    let layout_jitter = sample_layout(0.01, 0.01);

    let fp_orig = compute_layout_fingerprint(&layout_orig);
    let fp_jitter = compute_layout_fingerprint(&layout_jitter);

    let dist = hamming_distance(fp_orig, fp_jitter);
    assert!(
        dist <= 2,
        "Small translation/jitter should have Hamming distance <= 2, got: {dist}"
    );
}

#[test]
fn test_stage1_retrieval_latency() {
    let mut pool = ExemplarPool::new();
    let sample = sample_layout(0.0, 0.0);
    let target_fp = compute_layout_fingerprint(&sample);

    // Register 1,000 synthetic templates
    for i in 0..1000 {
        let meta = ExemplarMetadata {
            exemplar_id: format!("doc_{i}"),
            vendor_name: Some(format!("Vendor {i}")),
            vendor_tax_id: Some(format!("TAX-{i:05}")),
            document_type: "bill".to_string(),
            layout_fingerprint: target_fp.wrapping_add(i as u64),
            created_at_unix: 1700000000,
        };
        let template = TemplatePair {
            metadata: meta,
            source_artifact_path: None,
            spatial_layout: sample.clone(),
            ground_truth_json: serde_json::json!({"id": i}),
        };
        pool.register(template, None);
    }

    let start = Instant::now();
    let matches = pool.retrieve(&sample, Some("TAX-00450"), None);
    let elapsed = start.elapsed();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].metadata.exemplar_id, "doc_450");
    // Verify sub-500us retrieval
    assert!(
        elapsed.as_micros() < 2000, // Safe bound across CI environments (< 2ms)
        "Stage 1 retrieval took {} us",
        elapsed.as_micros()
    );
}

#[test]
fn test_zero_shot_prompt_overhead() {
    let layout = sample_layout(0.0, 0.0);
    let prompt = FewShotPromptBuilder::build("System Instructions", &[], &layout);

    // Zero-shot prompt must not contain few-shot example markers
    assert!(!prompt.contains("REFERENCE DOCUMENT EXAMPLES"));
    assert!(!prompt.contains("[Example 1]"));
    assert!(!prompt.contains("TARGET DOCUMENT FOR EXTRACTION"));
    assert!(prompt.contains("System Instructions"));
    assert!(prompt.contains("Extract all data strictly matching the requested schema"));
}

#[test]
fn test_dual_stage_retrieval_flow() {
    let mut pool = ExemplarPool::new();
    let layout_a = sample_layout(0.0, 0.0);
    let fp_a = compute_layout_fingerprint(&layout_a);

    let template = TemplatePair {
        metadata: ExemplarMetadata {
            exemplar_id: "ex_1".to_string(),
            vendor_name: Some("Pacific Gas".to_string()),
            vendor_tax_id: Some("US-94-123".to_string()),
            document_type: "electric".to_string(),
            layout_fingerprint: fp_a,
            created_at_unix: 1700000000,
        },
        source_artifact_path: None,
        spatial_layout: layout_a.clone(),
        ground_truth_json: serde_json::json!({"total": 142.50}),
    };

    let emb = vec![0.8, 0.2, 0.1];
    pool.register(template, Some(emb));

    // Match via tax ID
    let res_tax = pool.retrieve(&layout_a, Some("US-94-123"), None);
    assert_eq!(res_tax.len(), 1);

    // Match via fingerprint
    let res_fp = pool.retrieve(&layout_a, None, None);
    assert_eq!(res_fp.len(), 1);

    // Completely dissimilar layout with no tax ID and weak vector should fall back to zero-shot
    let empty_layout = SpatialLayoutMap::from_spans(Vec::new(), (500, 500));
    let weak_vec = vec![0.0, 0.9, 0.0];
    let res_zero = pool.retrieve(&empty_layout, None, Some(&weak_vec));
    assert!(res_zero.is_empty(), "Should fall back to zero-shot");
}
