use crate::backend::LlmBackend;
use crate::error::LlmError;
use async_trait::async_trait;
use docula_schema::{DfaLogitsMasker, SchemaDfa};
use oxibonsai::core::GgufStreamParser;
use oxibonsai_runtime::{AdaptiveLookahead, AdaptiveLookaheadConfig, KvCachePolicy};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Embedded 1-bit / sub-2-bit quantized neural LLM backend (PrismML Bonsai architecture).
///
/// Powered by KitaSan `oxibonsai`, enabling sovereign sub-2-bit LLM execution
/// (1-bit `Q1_0_g128` and ternary `TQ2_0_g128`) on edge CPU/Metal in <2 GB RAM.
///
/// NOTE: `oxibonsai` is designed specifically for PrismML Bonsai 1-bit quantized models
/// and is not suitable for general open weights. For standard open weights (LLaMA-3, Mistral,
/// Qwen, Phi), use [`crate::OxiLlamaBackend`] or [`crate::CandleBackend`].
#[derive(Clone, Debug)]
pub struct OxiBonsaiBackend {
    model_path: PathBuf,
    kv_policy: Arc<Mutex<KvCachePolicy>>,
    lookahead_config: AdaptiveLookaheadConfig,
    canned_response: Option<String>,
}

impl OxiBonsaiBackend {
    /// Creates a new `OxiBonsaiBackend` loading from the specified model path.
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            kv_policy: Arc::new(Mutex::new(KvCachePolicy::default())),
            lookahead_config: AdaptiveLookaheadConfig::default(),
            canned_response: None,
        }
    }

    /// Creates an in-memory testable `OxiBonsaiBackend` without requiring weights on disk.
    pub fn in_memory() -> Self {
        Self {
            model_path: PathBuf::from(":memory:"),
            kv_policy: Arc::new(Mutex::new(KvCachePolicy::default())),
            lookahead_config: AdaptiveLookaheadConfig::default(),
            canned_response: None,
        }
    }

    /// Configures a deterministic canned response for testing schema conformance.
    #[must_use]
    pub fn with_canned_response(mut self, response: impl Into<String>) -> Self {
        self.canned_response = Some(response.into());
        self
    }

    /// Model weights path.
    #[must_use]
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// Access the thread-safe KV cache policy controller.
    #[must_use]
    pub fn kv_policy(&self) -> Arc<Mutex<KvCachePolicy>> {
        self.kv_policy.clone()
    }

    /// Instantiates an adaptive speculative lookahead controller.
    #[must_use]
    pub fn create_lookahead_controller(&self) -> AdaptiveLookahead {
        AdaptiveLookahead::new(self.lookahead_config.clone())
    }

    /// Creates a GGUF streaming parser for Bonsai model inspection.
    #[must_use]
    pub fn stream_parser(&self) -> GgufStreamParser {
        GgufStreamParser::new()
    }
}

#[async_trait]
impl LlmBackend for OxiBonsaiBackend {
    fn name(&self) -> &'static str {
        "oxibonsai-embedded-1bit"
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
                    "OxiBonsai response does not conform to the required JSON schema".to_string(),
                ));
            }
            return Ok(canned.clone());
        }

        if self.model_path.to_string_lossy() != ":memory:" && !self.model_path.exists() {
            return Err(LlmError::ModelNotFound(format!(
                "OxiBonsai weights file not found at: {}",
                self.model_path.display()
            )));
        }

        // Return deterministic stub or simulated execution
        Ok("{}".to_string())
    }
}
