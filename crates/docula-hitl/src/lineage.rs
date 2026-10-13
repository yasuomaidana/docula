//! Human-in-the-loop revision lineage tracking using recursive CTEs.
//!
//! Provides `CorrectionLineageTracker` which persists human correction revisions and
//! traverses revision parent-child trees using `diesel-cte-ext`.

use crate::error::HitlError;
use diesel::sqlite::SqliteConnection;
use diesel::{Connection, RunQueryDsl, dsl::sql, sql_query, sql_types::Integer};
use diesel_cte_ext::{RecursiveCTEExt, RecursiveParts};
use serde::{Deserialize, Serialize};

/// Metadata record representing a specific revision in the HITL triage/correction chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageRevision {
    /// Unique revision identifier.
    pub id: i32,
    /// Parent revision identifier (or None if initial ingestion).
    pub parent_id: Option<i32>,
    /// Associated triage item identifier.
    pub item_id: String,
    /// Human reviewer or active-learning author.
    pub author: String,
    /// Summary of the JSON patch or field correction.
    pub patch_summary: String,
}

/// Correction lineage tracker utilizing `diesel-cte-ext` for recursive CTE queries.
pub struct CorrectionLineageTracker {
    conn: SqliteConnection,
}

impl CorrectionLineageTracker {
    /// Establishes an in-memory SQLite tracker.
    ///
    /// # Errors
    /// Returns `HitlError::Database` or `HitlError::Connection` on connection or schema init failure.
    pub fn in_memory() -> Result<Self, HitlError> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        Self::init_schema(&mut conn)?;
        Ok(Self { conn })
    }

    /// Establishes a tracker at the specified database URL or filesystem path.
    ///
    /// # Errors
    /// Returns `HitlError::Database` or `HitlError::Connection` on connection or schema init failure.
    pub fn open(database_url: &str) -> Result<Self, HitlError> {
        let mut conn = SqliteConnection::establish(database_url)?;
        Self::init_schema(&mut conn)?;
        Ok(Self { conn })
    }

    fn init_schema(conn: &mut SqliteConnection) -> Result<(), HitlError> {
        sql_query(
            "CREATE TABLE IF NOT EXISTS correction_revisions (
                id INTEGER PRIMARY KEY,
                parent_id INTEGER,
                item_id TEXT NOT NULL,
                author TEXT NOT NULL,
                patch_summary TEXT NOT NULL
            );",
        )
        .execute(conn)?;
        Ok(())
    }

    /// Inserts a new correction revision node into the lineage DAG.
    ///
    /// # Errors
    /// Returns `HitlError::Database` if insertion fails.
    pub fn insert_revision(
        &mut self,
        id: i32,
        parent_id: Option<i32>,
        item_id: &str,
        author: &str,
        patch_summary: &str,
    ) -> Result<(), HitlError> {
        let parent_val = match parent_id {
            Some(pid) => pid.to_string(),
            None => "NULL".to_string(),
        };
        let escaped_item = item_id.replace('\'', "''");
        let escaped_author = author.replace('\'', "''");
        let escaped_patch = patch_summary.replace('\'', "''");

        let insert_sql = format!(
            "INSERT INTO correction_revisions (id, parent_id, item_id, author, patch_summary) \
             VALUES ({id}, {parent_val}, '{escaped_item}', '{escaped_author}', '{escaped_patch}')"
        );
        sql_query(insert_sql).execute(&mut self.conn)?;
        Ok(())
    }

    /// Recursively traces the ancestor lineage from the target revision up to the root revision
    /// using `diesel-cte-ext` `with_recursive`.
    ///
    /// Returns IDs ordered from closest (target itself) to oldest ancestor (root).
    ///
    /// # Errors
    /// Returns `HitlError::Database` if query execution fails.
    pub fn get_ancestor_lineage(&mut self, start_id: i32) -> Result<Vec<i32>, HitlError> {
        let seed = format!("SELECT id, parent_id FROM correction_revisions WHERE id = {start_id}");
        let step = "SELECT r.id, r.parent_id FROM correction_revisions r \
                    INNER JOIN lineage l ON r.id = l.parent_id";
        let body = "SELECT id FROM lineage";

        let ancestors: Vec<i32> = self
            .conn
            .with_recursive(
                "lineage",
                &["id", "parent_id"],
                RecursiveParts::new(
                    sql::<Integer>(&seed),
                    sql::<Integer>(step),
                    sql::<Integer>(body),
                ),
            )
            .load(&mut self.conn)?;

        Ok(ancestors)
    }

    /// Recursively traces descendant revisions branching out from the given parent revision
    /// using `diesel-cte-ext` `with_recursive_not_all` (UNION distinct deduplication).
    ///
    /// # Errors
    /// Returns `HitlError::Database` if query execution fails.
    pub fn get_descendant_lineage(&mut self, root_id: i32) -> Result<Vec<i32>, HitlError> {
        let seed = format!("SELECT id, parent_id FROM correction_revisions WHERE id = {root_id}");
        let step = "SELECT r.id, r.parent_id FROM correction_revisions r \
                    INNER JOIN lineage l ON r.parent_id = l.id";
        let body = "SELECT id FROM lineage ORDER BY id ASC";

        let descendants: Vec<i32> = self
            .conn
            .with_recursive_not_all(
                "lineage",
                &["id", "parent_id"],
                RecursiveParts::new(
                    sql::<Integer>(&seed),
                    sql::<Integer>(step),
                    sql::<Integer>(body),
                ),
            )
            .load(&mut self.conn)?;

        Ok(descendants)
    }

    /// Computes the depth of a revision node (distance from the root revision).
    ///
    /// # Errors
    /// Returns `HitlError::Database` if query execution fails.
    pub fn get_revision_depth(&mut self, target_id: i32) -> Result<usize, HitlError> {
        let ancestors = self.get_ancestor_lineage(target_id)?;
        if ancestors.is_empty() {
            Ok(0)
        } else {
            Ok(ancestors.len() - 1)
        }
    }
}
