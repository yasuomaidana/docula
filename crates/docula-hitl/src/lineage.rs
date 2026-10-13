//! Human-in-the-loop revision lineage tracking using recursive CTEs.
//!
//! Provides `CorrectionLineageTracker` which persists human correction revisions and
//! traverses revision parent-child trees using `diesel-cte-ext` across both **SQLite**
//! and **PostgreSQL** relational databases.
//!
//! Developers have 100% control over their database schemas, table names, and column identifiers
//! via [`LineageTableConfig`] and the [`crate::lineage_table!`] macro.

use crate::error::HitlError;
use diesel::pg::PgConnection;
use diesel::sqlite::SqliteConnection;
use diesel::{Connection, RunQueryDsl, dsl::sql, sql_query, sql_types::Integer};
use diesel_cte_ext::{RecursiveCTEExt, RecursiveParts};
use serde::{Deserialize, Serialize};

/// Declarative macro for developers to define custom lineage tables following the Diesel DSL convention.
///
/// # Example
/// ```rust
/// use docula_hitl::lineage_table;
///
/// let config = lineage_table! {
///     audit_revisions (revision_id, parent_revision_id)
/// };
/// assert_eq!(config.table_name, "audit_revisions");
/// assert_eq!(config.id_column, "revision_id");
/// assert_eq!(config.parent_id_column, "parent_revision_id");
/// ```
#[macro_export]
macro_rules! lineage_table {
    ($table_name:ident ($id_col:ident, $parent_col:ident $(,)?)) => {
        $crate::LineageTableConfig::new(
            stringify!($table_name),
            stringify!($id_col),
            stringify!($parent_col),
        )
    };
}

/// Metadata configuration for a user-defined hierarchical lineage table.
///
/// Gives developers 100% control over table names and column identifiers
/// so existing production database migrations and schemas are preserved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageTableConfig {
    /// Table name in the database (e.g., "correction_revisions", "review_nodes", "audit_log").
    pub table_name: String,
    /// Primary key column (e.g., "id" or "revision_id").
    pub id_column: String,
    /// Self-referential parent key column (e.g., "parent_id" or "parent_revision_id").
    pub parent_id_column: String,
}

impl LineageTableConfig {
    /// Creates a custom lineage table configuration.
    pub fn new(
        table_name: impl Into<String>,
        id_column: impl Into<String>,
        parent_id_column: impl Into<String>,
    ) -> Self {
        Self {
            table_name: table_name.into(),
            id_column: id_column.into(),
            parent_id_column: parent_id_column.into(),
        }
    }
}

impl Default for LineageTableConfig {
    fn default() -> Self {
        Self::new("correction_revisions", "id", "parent_id")
    }
}

/// Internal connection variant supporting dual relational backends.
enum TrackerConnection {
    Sqlite(SqliteConnection),
    Postgres(PgConnection),
}

/// Metadata record representing a revision node in the HITL triage/correction chain.
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

/// Correction lineage tracker utilizing `diesel-cte-ext` for recursive CTE queries
/// on both SQLite and PostgreSQL without imposing rigid database schemas.
pub struct CorrectionLineageTracker {
    conn: TrackerConnection,
    config: LineageTableConfig,
}

impl CorrectionLineageTracker {
    /// Establishes an in-memory SQLite tracker with the default table configuration.
    ///
    /// # Errors
    /// Returns `HitlError::Database` or `HitlError::Connection` on connection failure.
    pub fn in_memory() -> Result<Self, HitlError> {
        Self::in_memory_with_config(LineageTableConfig::default())
    }

    /// Establishes an in-memory SQLite tracker with a user-defined table configuration.
    pub fn in_memory_with_config(config: LineageTableConfig) -> Result<Self, HitlError> {
        let conn = SqliteConnection::establish(":memory:")?;
        Ok(Self {
            conn: TrackerConnection::Sqlite(conn),
            config,
        })
    }

    /// Establishes a tracker at the specified database URL using default table settings.
    pub fn open(database_url: &str) -> Result<Self, HitlError> {
        Self::open_with_config(database_url, LineageTableConfig::default())
    }

    /// Establishes a tracker at the specified database URL using a user-defined table configuration.
    pub fn open_with_config(
        database_url: &str,
        config: LineageTableConfig,
    ) -> Result<Self, HitlError> {
        if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
            Self::open_postgres_with_config(database_url, config)
        } else {
            Self::open_sqlite_with_config(database_url, config)
        }
    }

    /// Establishes a tracker targeting an SQLite database with a custom configuration.
    pub fn open_sqlite_with_config(
        database_url: &str,
        config: LineageTableConfig,
    ) -> Result<Self, HitlError> {
        let conn = SqliteConnection::establish(database_url)?;
        Ok(Self {
            conn: TrackerConnection::Sqlite(conn),
            config,
        })
    }

    /// Establishes a tracker targeting a PostgreSQL server connection with a custom configuration.
    pub fn open_postgres_with_config(
        database_url: &str,
        config: LineageTableConfig,
    ) -> Result<Self, HitlError> {
        let conn = PgConnection::establish(database_url)?;
        Ok(Self {
            conn: TrackerConnection::Postgres(conn),
            config,
        })
    }

    /// Wraps an existing established SQLite connection with a custom table configuration.
    pub fn from_sqlite_connection(conn: SqliteConnection, config: LineageTableConfig) -> Self {
        Self {
            conn: TrackerConnection::Sqlite(conn),
            config,
        }
    }

    /// Wraps an existing established PostgreSQL connection with a custom table configuration.
    pub fn from_postgres_connection(conn: PgConnection, config: LineageTableConfig) -> Self {
        Self {
            conn: TrackerConnection::Postgres(conn),
            config,
        }
    }

    /// Returns a reference to the active table configuration.
    #[must_use]
    pub fn config(&self) -> &LineageTableConfig {
        &self.config
    }

    /// Returns the active relational database dialect ("sqlite" or "postgresql").
    #[must_use]
    pub fn database_type(&self) -> &'static str {
        match self.conn {
            TrackerConnection::Sqlite(_) => "sqlite",
            TrackerConnection::Postgres(_) => "postgresql",
        }
    }

    /// Inserts a revision node into the user-defined table using the configured columns.
    pub fn insert_revision(
        &mut self,
        id: i32,
        parent_id: Option<i32>,
        item_id: &str,
        author: &str,
        patch_summary: &str,
    ) -> Result<(), HitlError> {
        let table = &self.config.table_name;
        let id_col = &self.config.id_column;
        let parent_col = &self.config.parent_id_column;

        let parent_val = match parent_id {
            Some(pid) => pid.to_string(),
            None => "NULL".to_string(),
        };
        let escaped_item = item_id.replace('\'', "''");
        let escaped_author = author.replace('\'', "''");
        let escaped_patch = patch_summary.replace('\'', "''");

        let insert_sql = format!(
            "INSERT INTO {table} ({id_col}, {parent_col}, item_id, author, patch_summary) \
             VALUES ({id}, {parent_val}, '{escaped_item}', '{escaped_author}', '{escaped_patch}')"
        );

        match self.conn {
            TrackerConnection::Sqlite(ref mut c) => {
                sql_query(insert_sql).execute(c)?;
            }
            TrackerConnection::Postgres(ref mut c) => {
                sql_query(insert_sql).execute(c)?;
            }
        }
        Ok(())
    }

    /// Inserts a raw parent-child node pair into the user-defined lineage table.
    pub fn insert_node(&mut self, id: i32, parent_id: Option<i32>) -> Result<(), HitlError> {
        let table = &self.config.table_name;
        let id_col = &self.config.id_column;
        let parent_col = &self.config.parent_id_column;

        let parent_val = match parent_id {
            Some(pid) => pid.to_string(),
            None => "NULL".to_string(),
        };

        let insert_sql = format!(
            "INSERT INTO {table} ({id_col}, {parent_col}) \
             VALUES ({id}, {parent_val})"
        );

        match self.conn {
            TrackerConnection::Sqlite(ref mut c) => {
                sql_query(insert_sql).execute(c)?;
            }
            TrackerConnection::Postgres(ref mut c) => {
                sql_query(insert_sql).execute(c)?;
            }
        }
        Ok(())
    }

    /// Recursively traces the ancestor lineage from the target revision up to the root revision
    /// using `diesel-cte-ext` `with_recursive` over the developer-defined table and columns.
    ///
    /// Returns IDs ordered from closest (target itself) to oldest ancestor (root).
    pub fn get_ancestor_lineage(&mut self, start_id: i32) -> Result<Vec<i32>, HitlError> {
        let table = &self.config.table_name;
        let id_col = &self.config.id_column;
        let parent_col = &self.config.parent_id_column;

        let seed = format!("SELECT {id_col} AS id, {parent_col} AS parent_id FROM {table} WHERE {id_col} = {start_id}");
        let step = format!(
            "SELECT r.{id_col} AS id, r.{parent_col} AS parent_id FROM {table} r \
             INNER JOIN lineage l ON r.{id_col} = l.parent_id"
        );
        let body = "SELECT id FROM lineage";

        let ancestors: Vec<i32> = match self.conn {
            TrackerConnection::Sqlite(ref mut c) => c
                .with_recursive(
                    "lineage",
                    &["id", "parent_id"],
                    RecursiveParts::new(
                        sql::<Integer>(&seed),
                        sql::<Integer>(&step),
                        sql::<Integer>(body),
                    ),
                )
                .load(c)?,
            TrackerConnection::Postgres(ref mut c) => c
                .with_recursive(
                    "lineage",
                    &["id", "parent_id"],
                    RecursiveParts::new(
                        sql::<Integer>(&seed),
                        sql::<Integer>(&step),
                        sql::<Integer>(body),
                    ),
                )
                .load(c)?,
        };

        Ok(ancestors)
    }

    /// Recursively traces descendant revisions branching out from the given parent revision
    /// using `diesel-cte-ext` `with_recursive_not_all` (UNION distinct deduplication).
    pub fn get_descendant_lineage(&mut self, root_id: i32) -> Result<Vec<i32>, HitlError> {
        let table = &self.config.table_name;
        let id_col = &self.config.id_column;
        let parent_col = &self.config.parent_id_column;

        let seed = format!("SELECT {id_col} AS id, {parent_col} AS parent_id FROM {table} WHERE {id_col} = {root_id}");
        let step = format!(
            "SELECT r.{id_col} AS id, r.{parent_col} AS parent_id FROM {table} r \
             INNER JOIN lineage l ON r.{parent_col} = l.id"
        );
        let body = "SELECT id FROM lineage ORDER BY id ASC";

        let descendants: Vec<i32> = match self.conn {
            TrackerConnection::Sqlite(ref mut c) => c
                .with_recursive_not_all(
                    "lineage",
                    &["id", "parent_id"],
                    RecursiveParts::new(
                        sql::<Integer>(&seed),
                        sql::<Integer>(&step),
                        sql::<Integer>(body),
                    ),
                )
                .load(c)?,
            TrackerConnection::Postgres(ref mut c) => c
                .with_recursive_not_all(
                    "lineage",
                    &["id", "parent_id"],
                    RecursiveParts::new(
                        sql::<Integer>(&seed),
                        sql::<Integer>(&step),
                        sql::<Integer>(body),
                    ),
                )
                .load(c)?,
        };

        Ok(descendants)
    }

    /// Computes the depth of a revision node (distance from the root revision).
    pub fn get_revision_depth(&mut self, target_id: i32) -> Result<usize, HitlError> {
        let ancestors = self.get_ancestor_lineage(target_id)?;
        if ancestors.is_empty() {
            Ok(0)
        } else {
            Ok(ancestors.len() - 1)
        }
    }
}
