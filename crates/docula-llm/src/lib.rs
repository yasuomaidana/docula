//! # docula-llm
//!
//! Sovereign local-embedded and remote LLM backends with DFA logits masking for `docula`.
//!
//! Provides the [`LlmBackend`] trait and concrete strategies:
//! - [`MockLlmBackend`]: Deterministic mock backend with step-by-step logits masking evaluation.
//! - [`OxiLlamaBackend`]: Pure-Rust general GGUF quantized engine powered by KitaSan `oxillama`.
//! - [`OxiBonsaiBackend`]: Embedded 1-bit / sub-2-bit quantized pure-Rust engine powered by KitaSan `oxibonsai`.
//! - [`CandleBackend`]: Pure-Rust Hugging Face Candle GGUF/Safetensors engine.
//! - [`TractBackend`]: Pure-Rust ONNX neural runtime engine.

pub mod backend;
pub mod candle;
pub mod common;
pub mod error;
pub mod mock;
pub mod oxibonsai;
pub mod oxillama;
pub mod tract;

pub use backend::LlmBackend;
pub use candle::CandleBackend;
pub use error::LlmError;
pub use mock::MockLlmBackend;
pub use oxibonsai::OxiBonsaiBackend;
pub use oxillama::OxiLlamaBackend;
pub use tract::TractBackend;
