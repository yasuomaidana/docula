use docula_hitl::CorrectionLineageTracker;

#[test]
fn test_lineage_tracker_in_memory_and_ancestry() {
    let mut tracker = CorrectionLineageTracker::in_memory().expect("in-memory tracker");

    // Seed tree:
    // 1 (root) -> 2 -> 3 -> 4
    //          \-> 5
    tracker
        .insert_revision(1, None, "item-100", "annotator_1", "Initial draft")
        .expect("insert 1");
    tracker
        .insert_revision(2, Some(1), "item-100", "annotator_2", "Fixed invoice date")
        .expect("insert 2");
    tracker
        .insert_revision(3, Some(2), "item-100", "annotator_1", "Corrected subtotal")
        .expect("insert 3");
    tracker
        .insert_revision(4, Some(3), "item-100", "annotator_3", "Final approval patch")
        .expect("insert 4");
    tracker
        .insert_revision(5, Some(1), "item-100", "annotator_4", "Alternative branch")
        .expect("insert 5");

    // Ancestor lineage of 4: [4, 3, 2, 1]
    let ancestors = tracker.get_ancestor_lineage(4).expect("get ancestors");
    assert_eq!(ancestors, vec![4, 3, 2, 1]);

    // Depth check:
    assert_eq!(tracker.get_revision_depth(1).expect("depth 1"), 0);
    assert_eq!(tracker.get_revision_depth(2).expect("depth 2"), 1);
    assert_eq!(tracker.get_revision_depth(4).expect("depth 4"), 3);

    // Descendants of 2: [2, 3, 4]
    let descendants = tracker.get_descendant_lineage(2).expect("get descendants");
    assert_eq!(descendants, vec![2, 3, 4]);

    // Descendants of root 1: [1, 2, 3, 4, 5]
    let all_descendants = tracker.get_descendant_lineage(1).expect("get descendants of 1");
    assert_eq!(all_descendants, vec![1, 2, 3, 4, 5]);
}
