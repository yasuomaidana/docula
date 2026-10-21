use crate::error::LlmError;
use async_trait::async_trait;

/// Sovereign abstraction trait for local embedded and remote LLM inference engines.
#[async_trait]
pub trait LlmBackend: Send + Sync {
    /// Human-readable identifier and version of the backend engine.
    fn name(&self) -> &'static str;

    /// Executes autoregressive token generation constrained by a compiled JSON Schema grammar.
    async fn generate_constrained(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: &schemars::Schema,
    ) -> Result<String, LlmError>;
}
