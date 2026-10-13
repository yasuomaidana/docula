use crate::backend::LlmBackend;
use crate::error::LlmError;
use async_trait::async_trait;
use docula_schema::{DfaLogitsMasker, SchemaDfa};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Deterministic mock LLM backend for testing and verification without external neural weights.
#[derive(Clone, Debug, Default)]
pub struct MockLlmBackend {
    canned_response: Option<String>,
    tokens_generated_count: Arc<AtomicUsize>,
    logits_mask_evaluations: Arc<AtomicUsize>,
}

impl MockLlmBackend {
    /// Constructs a new `MockLlmBackend`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Constructs a mock backend with a fixed canned JSON response.
    #[must_use]
    pub fn with_canned_response(response: impl Into<String>) -> Self {
        Self {
            canned_response: Some(response.into()),
            tokens_generated_count: Arc::new(AtomicUsize::new(0)),
            logits_mask_evaluations: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Total number of logits masking evaluations performed.
    #[must_use]
    pub fn logits_mask_evaluations(&self) -> usize {
        self.logits_mask_evaluations.load(Ordering::SeqCst)
    }

    /// Total number of tokens generated across calls.
    #[must_use]
    pub fn tokens_generated_count(&self) -> usize {
        self.tokens_generated_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl LlmBackend for MockLlmBackend {
    fn name(&self) -> &'static str {
        "mock-deterministic-backend"
    }

    async fn generate_constrained(
        &self,
        _system_prompt: &str,
        _user_prompt: &str,
        schema: &schemars::Schema,
    ) -> Result<String, LlmError> {
        let dfa = SchemaDfa::compile(schema)?;
        let masker = DfaLogitsMasker::new(dfa);

        if let Some(ref response) = self.canned_response {
            // Verify that canned response is valid according to grammar
            if !masker.dfa().is_accepted(response) {
                return Err(LlmError::InferenceFailure(
                    "Canned response does not conform to the required JSON schema".to_string(),
                ));
            }
            return Ok(response.clone());
        }

        // Simulate step-by-step autoregressive token generation with logits masking
        let vocab = vec![
            "{\n".to_string(),
            "  \"id\": 1,\n".to_string(),
            "  \"name\": \"Sample\",\n".to_string(),
            "  \"total\": 100.0\n".to_string(),
            "}".to_string(),
            "invalid_token\n".to_string(),
        ];

        let mut current_text = String::new();
        let max_steps = 16;

        for step in 0..max_steps {
            let mut logits = vec![0.0; vocab.len()];
            if step < 5 {
                logits[step] = 10.0;
            }
            // Invalid token given artificially highest score; logits masker must suppress it
            logits[5] = 20.0;

            // Logits masking intercepted per token
            masker.mask_logits(&current_text, &mut logits, &vocab);
            self.logits_mask_evaluations.fetch_add(1, Ordering::SeqCst);

            // Greedily pick valid token
            if let Some(next_idx) = masker.sample_greedy(&current_text, &logits, &vocab) {
                current_text.push_str(&vocab[next_idx]);
                self.tokens_generated_count.fetch_add(1, Ordering::SeqCst);

                if masker.dfa().is_accepted(&current_text) {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(current_text)
    }
}
