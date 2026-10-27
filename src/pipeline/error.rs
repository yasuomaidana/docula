//! Pipeline error hierarchy.

use docula_composer::ComposerError;
use docula_core::DoculaError;
use docula_hitl::HitlError;
use docula_ingest::IngestError;
use docula_llm::LlmError;
use docula_ocr::OcrError;
use docula_preprocess::PreprocessError;
use docula_schema::SchemaError;
use docula_vision_codes::OpticalCodeError;
use thiserror::Error;

/// Typed pipeline failure hierarchy.
#[derive(Debug, Error)]
pub enum DoculaPipelineError {
    #[error("Core error: {0}")]
    Core(#[from] DoculaError),

    #[error("Preprocessing error: {0}")]
    Preprocess(#[from] PreprocessError),

    #[error("Ingest error: {0}")]
    Ingest(#[from] IngestError),

    #[error("OCR error: {0}")]
    Ocr(#[from] OcrError),

    #[error("Schema error: {0}")]
    Schema(#[from] SchemaError),

    #[error("LLM inference error: {0}")]
    Llm(#[from] LlmError),

    #[error("HITL error: {0}")]
    Hitl(#[from] HitlError),

    #[error("Optical code error: {0}")]
    VisionCodes(#[from] OpticalCodeError),

    #[error("Composer error: {0}")]
    Composer(#[from] ComposerError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Pipeline execution failure: {0}")]
    ExecutionFailure(String),
}
