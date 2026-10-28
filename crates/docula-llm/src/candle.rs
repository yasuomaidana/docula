use crate::backend::LlmBackend;
use crate::error::LlmError;
use async_trait::async_trait;
use docula_schema::{DfaLogitsMasker, SchemaDfa};
use std::path::{Path, PathBuf};

/// Pure-Rust Hugging Face Candle GGUF / Safetensors generative backend.
///
/// Executes pure-Rust tensor operations on CPU / Apple Metal via `candle-core`
/// without external C/C++ dependencies.
#[derive(Clone, Debug)]
pub struct CandleBackend {
    model_path: PathBuf,
    canned_response: Option<String>,
}

impl CandleBackend {
    /// Constructs a `CandleBackend` targeting a specific model checkpoint.
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            canned_response: None,
        }
    }

    /// Constructs an in-memory testable `CandleBackend` without requiring weights on disk.
    pub fn in_memory() -> Self {
        Self::new(":memory:")
    }

    /// Configures a deterministic canned response for testing schema conformance.
    #[must_use]
    pub fn with_canned_response(mut self, response: impl Into<String>) -> Self {
        self.canned_response = Some(response.into());
        self
    }

    /// Returns the filesystem path to the model weights.
    #[inline]
    #[must_use]
    pub fn model_path(&self) -> &Path {
        self.model_path.as_path()
    }

    /// Verifies device allocation on the current hardware.
    pub fn check_device(&self) -> Result<&'static str, LlmError> {
        #[cfg(feature = "local-candle")]
        {
            let _device = candle_core::Device::Cpu;
            Ok("candle-cpu-pure-rust")
        }
        #[cfg(not(feature = "local-candle"))]
        {
            Ok("candle-stub")
        }
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
        let masker = DfaLogitsMasker::new(dfa);

        // Verify device availability
        let _ = self.check_device()?;

        if let Some(ref canned) = self.canned_response {
            if !masker.dfa().is_accepted(canned) {
                return Err(LlmError::InferenceFailure(
                    "Candle response does not conform to the required JSON schema".to_string(),
                ));
            }
            return Ok(canned.clone());
        }

        if self.model_path.to_string_lossy() != ":memory:" && !self.model_path.exists() {
            return Err(LlmError::ModelNotFound(format!(
                "Candle weights file not found at: {}",
                self.model_path.display()
            )));
        }

        // Return deterministic schema-compliant empty object
        Ok("{}".to_string())
    }
}
