# Module 04: LLM & Local/Embedded Backends

## 1. Overview & Purpose

The **LLM & Local/Embedded Backends** module forms the computational backbone of Step 2 (filling structured data). It converts unstructured spatial text into structured objects.

### Core Design Philosophy: Sovereign, Local-First

- **Priority 1: Zero-FFI Sovereign Models (KitaSan Stack):** Run models directly within the Rust application address space using pure-Rust runtimes. No Python runtime, no Docker container, no external C/C++ dynamic libraries, no network calls.
- **Priority 2: Extreme Efficiency (General Quantized GGUF & Sub-2-Bit Inference):**
  - **`oxillama`**: Pure-Rust alternative to `llama.cpp` for loading and executing GGUF quantized models (Llama 3, Mistral, Qwen2.5, DeepSeek, Phi-3).
  - **`oxibonsai`**: Pure-Rust sub-2-bit (1-bit `Q1_0_g128` and ternary `TQ2_0_g128`) inference engine for running 7B–8B models on edge devices in <2 GB RAM.
- **Priority 3: Feature-Flag Modularity:** The user chooses their backend at compile-time via Cargo features (`oxillama`, `oxibonsai`, `candle`, `tract`, `remote-ollama`, `remote-openai`).

```mermaid
flowchart TD
    SpatialInput[Spatial Document Layout Text] --> EngineRouter[Trait: LlmBackend]
    
    subgraph SovereignPriority["Primary Backends: KitaSan Sovereign Stack (Zero FFI)"]
        direction TB
        OxiLLaMa["oxillama (Pure Rust llama.cpp Alternative: GGUF Q4_K, Q8_0)"]
        OxiBonsai["oxibonsai (Pure Rust 1-Bit & Ternary Q1_0_g128 / TQ2_0_g128)"]
        SciRS2["scirs2 (Pure-Rust SIMD Tensor & Math Acceleration)"]
    end

    subgraph SecondaryLocal["Secondary Local Engines (Zero FFI)"]
        Candle["candle / Crane (Safetensors / GGUF / VLMs)"]
        Tract["tract-onnx (Pure-Rust Embedded ONNX Runtime)"]
    end

    subgraph RemoteOptional["Optional Remote Gateways (Feature-Gated)"]
        Ollama["Local Ollama API (rig-core/ollama)"]
        CloudAPI["Cloud Endpoints: OpenAI/Anthropic (rig-core/openai)"]
    end

    EngineRouter -->|feature = 'local-gguf'| OxiLLaMa
    EngineRouter -->|feature = 'local-embedded' (default)| OxiBonsai
    EngineRouter -->|feature = 'local-candle'| Candle
    EngineRouter -->|feature = 'local-onnx'| Tract
    EngineRouter -->|feature = 'remote-ollama'| Ollama
    EngineRouter -->|feature = 'remote-openai'| CloudAPI
    OxiBonsai --> SciRS2
```

---

## 2. Technical Stack & Cargo Feature Flags

```toml
[features]
default = ["local-embedded"]

# Default Sovereign Local Engine (Sub-2-bit / 1-bit quantized PrismML Bonsai)
local-embedded = ["dep:oxibonsai", "dep:scirs2"]

# General Quantized GGUF Sovereign Engine (Pure-Rust llama.cpp alternative)
local-gguf     = ["dep:oxillama"]

# Hugging Face Pure-Rust Framework (GGUF, Safetensors, VLMs)
local-candle   = ["dep:candle-core", "dep:candle-transformers", "dep:candle-nn"]

# Pure-Rust ONNX Neural Runtime
local-onnx     = ["dep:tract-core", "dep:tract-onnx"]

# External / Local-Daemon Fallbacks
remote-ollama  = ["dep:rig-core", "rig-core/ollama"]
remote-openai  = ["dep:rig-core", "rig-core/openai"]
```

---

## 3. Deep Dive: KitaSan Sovereign LLM Engines

### 3.1 `oxillama` — Pure Rust Sovereign Alternative to `llama.cpp`

[`cool-japan/oxillama`](https://github.com/cool-japan/oxillama) provides complete GGUF model loading, multi-format quantized inference, and an OpenAI-compatible API server—**without a single line of C, C++, or Fortran code**:
- **Crate Topology**:
  - `oxillama-gguf`: Pure-Rust parser and loader for GGUF metadata, tensor headers, and quantization scales.
  - `oxillama-quant`: Pure-Rust dequantization and quantized dot-product kernels for `Q4_0`, `Q4_1`, `Q4_K_M`, `Q4_K_S`, `Q5_0`, `Q5_K`, `Q8_0`, `F16`, and `BF16`.
  - `oxillama-arch`: Architecture definitions for Llama 3, Mistral, Qwen2.5, DeepSeek-V2/V3, Gemma 2, and Phi-3/4.
  - `oxillama-runtime`: Autoregressive token generation loop, KV cache manager, and logits processor hooks.
  - `oxillama-server`: Embedded HTTP server implementing OpenAI `/v1/chat/completions` and `/v1/models`.
  - `oxillama-gpu`: Zero-FFI GPU acceleration via pure-Rust WebGPU/Metal and `oxicuda`.
- **Logits Processing for Constrained Decoding**: `oxillama-runtime` natively accepts custom logits interceptors, enabling `docula-schema` to apply DFA token bitmasks directly during token sampling.

### 3.2 `oxibonsai` — Pure Rust 1-Bit & Ternary Inference Engine

[`cool-japan/oxibonsai`](https://github.com/cool-japan/oxibonsai) is a zero-FFI, zero-C/C++ inference engine specifically tuned for PrismML's sub-2-bit Bonsai models:
- **Quantization Families**:
  - 1-Bit (`Q1_0_g128`): Extreme compression; replaces matrix multiplications with bit-scatter operations where single bits expand to $\pm\text{scale}$.
  - Ternary (`TQ2_0_g128`): $\{-1, 0, 1\}$ weights delivering superior perplexity at fractional memory footprints.
- **Memory Footprint**: An 8B parameter model requires only ~1.15 GB disk storage and executes inside <2 GB RAM.
- **Runtime Controllers**:
  - `KvCachePolicy`: Adaptive EWMA memory pressure tracking.
  - `AdaptiveLookahead`: Dynamic speculative-decoding lookahead length optimization.
- **Hardware Acceleration**: Built on `SciRS2` tensor math, leveraging CPU AVX2/AVX-512/NEON SIMD and Apple Silicon Metal without external BLAS or C compilers.

### 3.3 `candle` & `Crane` (Pure-Rust LLM & VLM Runtime)

Hugging Face's `candle` is supported as an alternative pure-Rust engine for running Vision-Language Models (e.g. Qwen2.5-VL and Florence-2) when documents require multi-modal visual patch tokens.

### 3.4 `tract` (Pure-Rust ONNX Runtime)

Sonos's `tract` is 100% safe Rust and executes ONNX-exported encoder-decoder and small generative models with zero external C dynamic libraries.

---

## 4. Trait Abstraction: `LlmBackend`

To guarantee pluggability across local and remote providers, `docula` defines a unified trait:

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

### 4.1 Implementation Example: `oxillama` Backend (General GGUF)

```rust
#[cfg(feature = "local-gguf")]
use oxillama_runtime::{Engine as OxiLlamaEngine, GenerationConfig};

#[cfg(feature = "local-gguf")]
pub struct OxiLlamaBackend {
    engine: OxiLlamaEngine,
}

#[cfg(feature = "local-gguf")]
#[async_trait]
impl LlmBackend for OxiLlamaBackend {
    fn name(&self) -> &'static str {
        "oxillama-gguf-pure-rust"
    }

    async fn generate_constrained(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: &RootSchema,
    ) -> Result<String, LlmError> {
        let grammar_dfa = typedlm::schema_to_dfa(schema)?;
        let logits_processor = typedlm::DfaLogitsProcessor::new(grammar_dfa);
        
        let prompt = format!(
            "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\nInput Document:\n{}\n\nStrict Schema:\n{}<|im_end|>\n<|im_start|>assistant\n",
            system_prompt,
            user_prompt,
            serde_json::to_string(schema)?
        );

        let gen_config = GenerationConfig {
            max_tokens: 1024,
            temperature: 0.0, // Greedy sampling for deterministic extraction
            ..Default::default()
        };

        let output = self.engine.generate_with_logits_interceptor(
            &prompt,
            gen_config,
            logits_processor,
        )?;

        Ok(output)
    }
}
```

### 4.2 Implementation Example: `oxibonsai` Backend (Sub-2-Bit Edge)

```rust
#[cfg(feature = "local-embedded")]
use oxibonsai_runtime::{EngineBuilder, SamplingPreset};

#[cfg(feature = "local-embedded")]
pub struct OxiBonsaiBackend {
    model_path: std::path::PathBuf,
}

#[cfg(feature = "local-embedded")]
#[async_trait]
impl LlmBackend for OxiBonsaiBackend {
    fn name(&self) -> &'static str {
        "oxibonsai-sub2bit-pure-rust"
    }

    async fn generate_constrained(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: &RootSchema,
    ) -> Result<String, LlmError> {
        let engine = EngineBuilder::new()
            .model_path(&self.model_path)
            .preset(SamplingPreset::Deterministic)
            .max_seq_len(4096)
            .build()?;

        let grammar_dfa = typedlm::schema_to_dfa(schema)?;
        let prompt = format!(
            "<|system|>\n{}\n<|schema|>\n{}\n<|user|>\n{}\n<|assistant|>\n",
            system_prompt,
            serde_json::to_string(schema)?,
            user_prompt
        );

        let output = engine.generate_with_logits_processor(
            &prompt,
            typedlm::DfaLogitsProcessor::new(grammar_dfa),
            512,
        )?;

        Ok(output)
    }
}
```
