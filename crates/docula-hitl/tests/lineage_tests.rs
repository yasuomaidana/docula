use diesel::sqlite::SqliteConnection;
use diesel::{Connection, RunQueryDsl, sql_query};
use docula_hitl::{CorrectionLineageTracker, lineage_table};

#[test]
fn test_lineage_tracker_in_memory_and_ancestry() {
    let mut conn = SqliteConnection::establish(":memory:").expect("sqlite memory");
    sql_query(
        "CREATE TABLE IF NOT EXISTS correction_revisions (
            id INTEGER PRIMARY KEY,
            parent_id INTEGER,
            item_id TEXT NOT NULL,
            author TEXT NOT NULL,
            patch_summary TEXT NOT NULL
        );",
    )
    .execute(&mut conn)
    .expect("create table");

    let config = lineage_table! {
        correction_revisions (id, parent_id)
    };

    let mut tracker = CorrectionLineageTracker::from_sqlite_connection(conn, config);

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

    assert_eq!(tracker.database_type(), "sqlite");
}

#[test]
fn test_lineage_table_macro_and_custom_schema() {
    let mut conn = SqliteConnection::establish(":memory:").expect("sqlite memory");
    sql_query(
        "CREATE TABLE custom_review_dag (
            rev_node_id INTEGER PRIMARY KEY,
            parent_rev_node_id INTEGER
        );",
    )
    .execute(&mut conn)
    .expect("create custom table");

    let config = lineage_table! {
        custom_review_dag (rev_node_id, parent_rev_node_id)
    };
    assert_eq!(config.table_name, "custom_review_dag");
    assert_eq!(config.id_column, "rev_node_id");
    assert_eq!(config.parent_id_column, "parent_rev_node_id");

    let mut tracker = CorrectionLineageTracker::from_sqlite_connection(conn, config);

    tracker.insert_node(10, None).expect("insert 10");
    tracker.insert_node(20, Some(10)).expect("insert 20");
    tracker.insert_node(30, Some(20)).expect("insert 30");

    let ancestors = tracker.get_ancestor_lineage(30).expect("ancestors");
    assert_eq!(ancestors, vec![30, 20, 10]);

    let descendants = tracker.get_descendant_lineage(10).expect("descendants");
    assert_eq!(descendants, vec![10, 20, 30]);
}

#[test]
fn test_lineage_tracker_postgres_url_dispatch() {
    let result = CorrectionLineageTracker::open("postgresql://invalid_host:5432/nonexistent");
    assert!(
        result.is_err(),
        "Non-existent PostgreSQL server should return connection error"
    );
}
