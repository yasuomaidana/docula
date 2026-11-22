# Module 04: LLM & Local/Embedded Backends

## 1. Overview & Purpose

The **LLM & Local/Embedded Backends** module forms the computational backbone of Step 2 (filling structured data). It converts unstructured spatial text into structured objects.

### Core Design Philosophy: Sovereign, Local-First

- **Priority 1: Zero-FFI Sovereign Models:** Run models directly within the Rust application address space using pure-Rust runtimes. No Python runtime, no Docker container, no external C/C++ dynamic libraries, no network calls.
- **Priority 2: Extreme Efficiency (Hugging Face Candle & Tract):**
  - **`candle`**: Pure-Rust tensor and LLM inference engine supporting GGUF, Safetensors, and quantized checkpoints.
  - **`tract`**: Pure-Rust neural runtime engine for ONNX models.
  - *Licence Change Notice (2026-11)*: `oxillama` and `oxibonsai` were removed following their relicensing to BUSL-1.1 and repository privatization; `candle` and `tract` are used instead.
- **Priority 3: Feature-Flag Modularity:** The user chooses their backend at compile-time via Cargo features (`candle`, `tract`, `remote-ollama`, `remote-openai`).

```mermaid
flowchart TD
    SpatialInput[Spatial Document Layout Text] --> EngineRouter[Trait: LlmBackend]
    
    subgraph PrimaryLocal["Primary Local Engines (Zero FFI)"]
        direction TB
        Candle["candle (Hugging Face Pure-Rust: Safetensors / GGUF)"]
        Tract["tract-onnx (Pure-Rust Embedded ONNX Runtime)"]
        Notice["oxillama & oxibonsai: Removed (2026-11 BUSL-1.1 licence change)"]
    end

    subgraph RemoteOptional["Optional Remote Gateways (Feature-Gated)"]
        Ollama["Local Ollama API (rig-core/ollama)"]
        CloudAPI["Cloud Endpoints: OpenAI/Anthropic (rig-core/openai)"]
    end

    EngineRouter -->|feature = 'local-candle' (default)| Candle
    EngineRouter -->|feature = 'local-onnx'| Tract
    EngineRouter -->|feature = 'remote-ollama'| Ollama
    EngineRouter -->|feature = 'remote-openai'| CloudAPI
```

---

## 2. Technical Stack & Cargo Feature Flags

```toml
[features]
default = ["local-embedded", "local-candle"]

# Local embedded inference (oxibonsai was removed due to 2026-11 licence change)
local-embedded = []

# Hugging Face Pure-Rust Framework (GGUF, Safetensors)
local-candle   = ["dep:candle-core"]

# Pure-Rust ONNX Neural Runtime (oxillama was removed due to 2026-11 licence change)
local-onnx     = []

# External / Local-Daemon Fallbacks
remote-ollama  = []
remote-openai  = []
```

---

## 3. Sovereign LLM Engines

### 3.1 Licence Migration Notice (2026-11)

In November 2026, COOLJAPAN relicensed `oxillama` and `oxibonsai` to BUSL-1.1 and made their source repositories private. To preserve zero-FFI open-source sovereignty and permissive licensing, both `oxillama` and `oxibonsai` were permanently removed from `docula-llm`. All inference workloads are now powered by pure-Rust permissive alternatives: **`candle`** and **`tract`**.

### 3.2 `candle` (Pure-Rust Hugging Face Runtime)

Hugging Face's `candle` is the primary local pure-Rust engine for running quantized models (GGUF and Safetensors checkpoints):
- 100% pure Rust tensor library with optional CPU and Metal/CUDA acceleration.
- Zero C/C++ build requirements.
- Native support for constrained generation and DFA logits masking.

### 3.3 `tract` (Pure-Rust ONNX Runtime)

Sonos's `tract` is 100% safe Rust and executes ONNX-exported neural models with zero external C dynamic libraries.

---

## 4. Trait Abstraction: `LlmBackend`

To guarantee pluggability across local and remote providers, `docula` defines a unified trait:

```rust
use async_trait::async_trait;
use schemars::Schema;

#[async_trait]
pub trait LlmBackend: Send + Sync {
    /// Name and version of the backend engine
    fn name(&self) -> &'static str;

    /// Execute completion with grammar / logits constraints enforcing the given schema
    async fn generate_constrained(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: &Schema,
    ) -> Result<String, LlmError>;
}
```

### 4.1 Implementation: `CandleBackend`

```rust
use docula_llm::{CandleBackend, LlmBackend};

let backend = CandleBackend::new("models/model.gguf");
assert_eq!(backend.name(), "candle-pure-rust-gguf");
```

### 4.2 Implementation: `TractBackend`

```rust
use docula_llm::{TractBackend, LlmBackend};

let backend = TractBackend::new("models/model.onnx");
assert_eq!(backend.name(), "tract-onnx-pure-rust");
```
