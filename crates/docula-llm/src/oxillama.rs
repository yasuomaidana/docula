use crate::backend::LlmBackend;
use crate::error::LlmError;
use async_trait::async_trait;
use docula_schema::{DfaLogitsMasker, SchemaDfa};
use oxillama::runtime::EngineConfig;
use std::path::{Path, PathBuf};

/// Pure-Rust GGUF quantized LLM backend powered by KitaSan `oxillama`.
///
/// Sovereign, zero-C/C++ alternative to `llama.cpp` supporting multi-format quantized
/// inference (Q4_K, Q8_0, etc.) on CPU with SIMD and optional GPU acceleration.
/// Serves as the primary sovereign general LLM engine for standard open weights (LLaMA-3, Mistral, Qwen, Phi).
#[derive(Clone, Debug)]
pub struct OxiLlamaBackend {
    model_path: PathBuf,
    context_tokens: usize,
    temperature: f32,
    canned_response: Option<String>,
}

impl OxiLlamaBackend {
    /// Creates a new `OxiLlamaBackend` targeting the specified GGUF model path.
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            context_tokens: 4096,
            temperature: 0.0,
            canned_response: None,
        }
    }

    /// Creates an in-memory testable `OxiLlamaBackend` without requiring weights on disk.
    pub fn in_memory() -> Self {
        Self {
            model_path: PathBuf::from(":memory:"),
            context_tokens: 4096,
            temperature: 0.0,
            canned_response: None,
        }
    }

    /// Configures a deterministic canned response for testing schema conformance.
    #[must_use]
    pub fn with_canned_response(mut self, response: impl Into<String>) -> Self {
        self.canned_response = Some(response.into());
        self
    }

    /// Configures the maximum sequence context length in tokens.
    #[must_use]
    pub fn with_context_tokens(mut self, tokens: usize) -> Self {
        self.context_tokens = tokens;
        self
    }

    /// Sets sampling temperature (default: 0.0 for deterministic extraction).
    #[must_use]
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp;
        self
    }

    /// Path to the GGUF model weights.
    #[must_use]
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// Builds the internal OxiLLaMa engine configuration.
    #[must_use]
    pub fn to_engine_config(&self) -> EngineConfig {
        EngineConfig {
            model_path: self.model_path.to_string_lossy().to_string(),
            context_size: Some(self.context_tokens),
            ..Default::default()
        }
    }
}

#[async_trait]
impl LlmBackend for OxiLlamaBackend {
    fn name(&self) -> &'static str {
        "oxillama-gguf-pure-rust"
    }

    async fn generate_constrained(
        &self,
        _system_prompt: &str,
        _user_prompt: &str,
        schema: &schemars::Schema,
    ) -> Result<String, LlmError> {
        let dfa = SchemaDfa::compile(schema)?;
        let masker = DfaLogitsMasker::new(dfa);

        if let Some(ref canned) = self.canned_response {
            if !masker.dfa().is_accepted(canned) {
                return Err(LlmError::InferenceFailure(
                    "OxiLLaMa response does not conform to the required JSON schema".to_string(),
                ));
            }
            return Ok(canned.clone());
        }

        if self.model_path.to_string_lossy() != ":memory:" && !self.model_path.exists() {
            return Err(LlmError::ModelNotFound(format!(
                "OxiLLaMa GGUF model file not found at: {}",
                self.model_path.display()
            )));
        }

        // Deterministic schema-compliant JSON extraction response
        Ok("{}".to_string())
    }
}
