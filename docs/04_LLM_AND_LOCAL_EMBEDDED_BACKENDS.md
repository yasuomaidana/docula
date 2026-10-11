# Module 04: LLM & Local/Embedded Backends

## 1. Overview & Purpose

The **LLM & Local/Embedded Backends** module forms the computational backbone of Step 2 (filling structured data). It converts unstructured spatial text into structured objects.

### Core Design Philosophy: Sovereign, Local-First
- **Priority 1: Zero-FFI Embedded Models:** Run models directly within the Rust application address space using pure-Rust runtimes. No python runtime, no docker container, no network calls.
- **Priority 2: Extreme Efficiency (1-bit / Sub-2-bit & GGUF Quantization):** Enable running 7B-8B parameter models on low-power edge machines, consumer laptops, and air-gapped workstations using under 2 GB RAM.
- **Priority 3: Feature-Flag Modularity:** The user chooses their backend at compile-time via Cargo features (e.g. `oxibonsai`, `candle`, `tract`, `remote-ollama`, `remote-openai`).

```mermaid
flowchart TD
    SpatialInput[Spatial Document Layout Text] --> EngineRouter[Trait: StructuredExtractorBackend]
    
    subgraph LocalPriority["Primary Backends: Local & Embedded (Zero FFI)"]
        direction TB
        Bonsai["oxibonsai (Kitasan Stack: 1-bit / Ternary Q1_0_g128)"]
        Candle["candle / Crane (Safetensors / GGUF / VLMs)"]
        Tract["tract-onnx (Pure-Rust Embedded ONNX Runtime)"]
    end

    subgraph RemoteOptional["Optional Backends (Feature-Gated)"]
        Ollama["Local Ollama API (rig-core/ollama)"]
        CloudAPI["Cloud Endpoints: OpenAI/Anthropic (rig-core/openai)"]
    end

    EngineRouter -->|feature = 'local-embedded' (default)| Bonsai
    EngineRouter -->|feature = 'local-candle'| Candle
    EngineRouter -->|feature = 'local-onnx'| Tract
    EngineRouter -->|feature = 'remote-ollama'| Ollama
    EngineRouter -->|feature = 'remote-openai'| CloudAPI
```

---

## 2. Technical Stack & Cargo Feature Flags

```toml
[features]
default = ["local-embedded"]

# Default Sovereign Local Engine (Sub-2-bit / 1-bit quantized PrismML Bonsai)
local-embedded = ["dep:oxibonsai", "dep:scirs2"]

# Hugging Face Pure-Rust Framework (GGUF, Safetensors, VLMs)
local-candle   = ["dep:candle-core", "dep:candle-transformers", "dep:candle-nn"]

# Pure-Rust ONNX Neural Runtime
local-onnx     = ["dep:tract-core", "dep:tract-onnx"]

# External / Local-Daemon Fallbacks
remote-ollama  = ["dep:rig-core", "rig-core/ollama"]
remote-openai  = ["dep:rig-core", "rig-core/openai"]
```

---

## 3. Deep Dive: Embedded Local Engines

### 3.1 `oxibonsai` (Kitasan Stack Sovereign Engine)
- **Model Family:** PrismML Bonsai sub-2-bit / 1-bit quantized models (e.g., `Q1_0_g128`).
- **Memory Footprint:** 8B parameter model fits in ~1.15 GB disk and <2 GB RAM.
- **Arithmetic:** Replaces heavy matrix multiplication units with bit-scatter operations (where single bits expand to $\pm\text{scale}$).
- **Acceleration:** Pure-Rust `core::arch` SIMD (AVX-512 / NEON) or runtime PTX generation via `oxicuda` without requiring the NVIDIA `nvcc` compiler.

### 3.2 `candle` & `Crane` (Pure-Rust LLM & VLM Runtime)
- **Model Support:** Llama-3, Mistral, Qwen2.5, Phi-3 in quantized GGUF or Safetensors formats.
- **Vision-Language Models (VLMs):** Supports Qwen2.5-VL and Florence-2 via the pure-Rust Crane inference wrapper for end-to-end visual reasoning when spatial layout text alone is insufficient.

### 3.3 `tract` (Pure-Rust ONNX Runtime)
- Developed by Sonos, `tract` is 100% safe Rust and executes ONNX-exported encoder-decoder and small generative models with zero external C dynamic libraries.

---

## 4. Trait Abstraction: `LlmBackend`

To guarantee pluggability across local and remote providers, `invorc` defines a unified trait:

```rust
use async_trait::async_trait;
use schemars::schema::RootSchema;

#[async_trait]
pub trait LlmBackend: Send + Sync {
    /// Name and version of the backend engine
    fn name(&self) -> &'static str;

    /// Execute completion with grammar / logits constraints enforcing the given schema
    async fn generate_constrained(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: &RootSchema,
    ) -> Result<String, LlmError>;
}
```

### 4.1 Implementation Example: Embedded `oxibonsai` Backend

```rust
#[cfg(feature = "local-embedded")]
pub struct OxiBonsaiBackend {
    model: oxibonsai::ModelSession,
}

#[cfg(feature = "local-embedded")]
#[async_trait]
impl LlmBackend for OxiBonsaiBackend {
    fn name(&self) -> &'static str {
        "oxibonsai-embedded-1bit"
    }

    async fn generate_constrained(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: &RootSchema,
    ) -> Result<String, LlmError> {
        let grammar_dfa = typedlm::schema_to_dfa(schema)?;
        
        let prompt = format!(
            "<|system|>\n{}\n<|schema|>\n{}\n<|user|>\n{}\n<|assistant|>\n",
            system_prompt,
            serde_json::to_string(schema)?,
            user_prompt
        );
        
        let output_tokens = self.model.generate_with_logits_processor(
            &prompt,
            typedlm::DfaLogitsProcessor::new(grammar_dfa),
            512, // Max tokens
        )?;
        
        Ok(output_tokens)
    }
}
```

### 4.2 Implementation Example: `rig-core` Gateway Backend

```rust
#[cfg(feature = "remote-ollama")]
pub struct OllamaBackend {
    client: rig::providers::ollama::Client,
    model_name: String,
}

#[cfg(feature = "remote-ollama")]
#[async_trait]
impl LlmBackend for OllamaBackend {
    fn name(&self) -> &'static str {
        "ollama-local-api"
    }

    async fn generate_constrained(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: &RootSchema,
    ) -> Result<String, LlmError> {
        let agent = self.client.agent(&self.model_name)
            .preamble(system_prompt)
            .build();
            
        let prompt = format!(
            "Input Document:\n{}\n\nStrict JSON Schema:\n{}",
            user_prompt,
            serde_json::to_string(schema)?
        );
        
        let response = agent.prompt(&prompt).await?;
        Ok(response)
    }
}
```
