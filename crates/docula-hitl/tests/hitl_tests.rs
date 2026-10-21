use chrono::Utc;
use docula_core::{BoundingBox, SpatialSpan};
use docula_hitl::{
    ColdArchiveStore, CorrectionTriad, JsonPatchDiff, PromotionPolicy, ThresholdPromotionPolicy,
    TriadMetadata, TriageDecision, TriageGate, TriageItem, TriageQueue,
};
use docula_ocr::SpatialLayoutMap;
use docula_schema::ValidationError;
use tempfile::tempdir;
use uuid::Uuid;

fn sample_triad(vendor: Option<&str>, fp: u64) -> CorrectionTriad {
    let bbox = BoundingBox::from_coords_clamped(0.1, 0.1, 0.9, 0.3);
    let span = SpatialSpan::new("Test Document".to_string(), bbox, 0, 0.99).unwrap();
    let layout = SpatialLayoutMap::from_spans(vec![span], (1000, 1000));

    let bad_json = serde_json::json!({ "total": 100.0, "tax": 10.0, "subtotal": 80.0 });
    let corrected_json = serde_json::json!({ "total": 100.0, "tax": 10.0, "subtotal": 90.0 });
    let diff = JsonPatchDiff::compute(&bad_json, &corrected_json);

    let meta = TriadMetadata {
        vendor_id: vendor.map(String::from),
        layout_fingerprint: fp,
        user_id: Some("reviewer_alice".to_string()),
        review_latency_ms: 4500,
        captured_at: Utc::now(),
    };

    CorrectionTriad {
        triad_id: Uuid::now_v7(),
        source_artifact_sha256: CorrectionTriad::hash_artifact(b"DUMMY_PDF_BYTES"),
        intermediate_ocr_layout: layout,
        model_output_bad: bad_json,
        gate_errors: vec![ValidationError::new(
            "subtotal",
            "Arithmetic mismatch: subtotal + tax != total",
        )],
        ground_truth_corrected: corrected_json,
        diff,
        metadata: meta,
    }
}

#[test]
fn test_triad_serialization_and_hash_integrity() {
    let triad = sample_triad(Some("Acme Inc"), 0xDEADBEEF);

    let hash = triad.compute_record_hash();
    assert_eq!(hash.len(), 64);

    let serialized = serde_json::to_string(&triad).unwrap();
    let deserialized: CorrectionTriad = serde_json::from_str(&serialized).unwrap();
    assert_eq!(triad, deserialized);
}

#[test]
fn test_invoice_math_failure_routes_to_triage() {
    let gate = TriageGate::new(0.85);

    let domain_errors = vec![ValidationError::new(
        "total",
        "Arithmetic error: 10 + 80 != 100",
    )];

    let decision = gate.evaluate(0.95, &domain_errors);
    match decision {
        TriageDecision::NeedsReview { reasons } => {
            assert_eq!(reasons.len(), 1);
            assert!(reasons[0].contains("Arithmetic error"));
        }
        TriageDecision::Pass => panic!("Expected NeedsReview, got Pass"),
    }
}

#[test]
fn test_low_confidence_routes_to_triage() {
    let gate = TriageGate::new(0.85);

    let decision = gate.evaluate(0.70, &[]);
    match decision {
        TriageDecision::NeedsReview { reasons } => {
            assert!(reasons[0].contains("Token confidence"));
        }
        TriageDecision::Pass => panic!("Expected NeedsReview"),
    }
}

#[test]
fn test_triage_queue_fifo() {
    let queue = TriageQueue::new();
    assert_eq!(queue.pending_count(), 0);

    let layout = SpatialLayoutMap::from_spans(Vec::new(), (100, 100));
    let item = TriageItem {
        id: Uuid::now_v7(),
        model_output: serde_json::json!({"error": true}),
        reasons: vec!["Low confidence".to_string()],
        spatial_layout: layout,
        confidence: 0.65,
    };

    queue.enqueue(item.clone());
    assert_eq!(queue.pending_count(), 1);

    let popped = queue.dequeue().expect("item in queue");
    assert_eq!(popped.id, item.id);
    assert_eq!(queue.pending_count(), 0);
}

#[test]
fn test_promotion_policy_blocks_single_occurrence() {
    let policy = ThresholdPromotionPolicy::new(3);
    let triad = sample_triad(Some("VendorA"), 0x1234);

    // Recurrence count 1 should be blocked (< 3)
    let qualified = policy.evaluate_promotion(&triad, 1, &[]);
    assert!(!qualified, "Single occurrence must not be promoted");
}

#[test]
fn test_promotion_policy_accepts_recurring_triad() {
    let policy = ThresholdPromotionPolicy::new(3);
    let triad = sample_triad(Some("VendorA"), 0x1234);

    // Recurrence count 3 qualifies
    let qualified = policy.evaluate_promotion(&triad, 3, &[]);
    assert!(qualified, "Recurring pattern must qualify for promotion");

    let promoted = policy.promote(&triad, "invoice").unwrap();
    assert_eq!(promoted.metadata.document_type, "invoice");
    assert_eq!(
        promoted.metadata.layout_fingerprint,
        triad.metadata.layout_fingerprint
    );
}

#[test]
fn test_cold_archive_append_and_read_back() {
    let dir = tempdir().unwrap();
    let store = ColdArchiveStore::new(dir.path()).unwrap();

    let triad1 = sample_triad(Some("Vendor 1"), 0x1111);
    let triad2 = sample_triad(Some("Vendor 2"), 0x2222);

    store.append(&triad1).unwrap();
    store.append(&triad2).unwrap();

    let all = store.read_all().unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].triad_id, triad1.triad_id);
    assert_eq!(all[1].triad_id, triad2.triad_id);

    let count = store.count_by_fingerprint(0x1111).unwrap();
    assert_eq!(count, 1);
}
