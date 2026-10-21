use thiserror::Error;

/// Errors arising during triage gating, triad capture, lineage resolution, or cold archive storage.
#[derive(Error, Debug)]
pub enum HitlError {
    /// Ingestion or validation gate rejection.
    #[error("Triage gate rejection: {0}")]
    TriageRejection(String),

    /// File I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Promotion blocked.
    #[error("Promotion failed: {0}")]
    PromotionBlocked(String),

    /// Database query failure.
    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),

    /// Database connection failure.
    #[error("Database connection error: {0}")]
    Connection(#[from] diesel::ConnectionError),
}
