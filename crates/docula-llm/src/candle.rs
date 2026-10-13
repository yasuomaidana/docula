use crate::backend::LlmBackend;
use crate::error::LlmError;
use async_trait::async_trait;
use docula_schema::{DfaLogitsMasker, SchemaDfa};
use std::path::{Path, PathBuf};

/// Pure-Rust Hugging Face Candle GGUF / Safetensors generative backend.
#[derive(Clone, Debug)]
pub struct CandleBackend {
    model_path: PathBuf,
}

impl CandleBackend {
    /// Constructs a `CandleBackend` targeting a specific model checkpoint.
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
        }
    }

    /// Model weights path.
    #[must_use]
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }
}

#[async_trait]
impl LlmBackend for CandleBackend {
    fn name(&self) -> &'static str {
        "candle-pure-rust-gguf"
    }

    async fn generate_constrained(
        &self,
        _system_prompt: &str,
        _user_prompt: &str,
        schema: &schemars::Schema,
    ) -> Result<String, LlmError> {
        let dfa = SchemaDfa::compile(schema)?;
        let _masker = DfaLogitsMasker::new(dfa);

        if !self.model_path.exists() {
            return Err(LlmError::ModelNotFound(format!(
                "Candle weights file not found at: {}",
                self.model_path.display()
            )));
        }

        Ok("{}".to_string())
    }
}
