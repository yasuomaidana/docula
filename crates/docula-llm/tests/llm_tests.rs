use docula_llm::{
    CandleBackend, LlmBackend, MockLlmBackend, OxiBonsaiBackend, OxiLlamaBackend, TractBackend,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(JsonSchema, Serialize, Deserialize, Debug, PartialEq)]
struct SimpleRecord {
    id: u32,
    name: String,
    total: f64,
}

#[tokio::test]
async fn test_mock_llm_backend() {
    let canned = r#"{"id": 1, "name": "Acme Invoice", "total": 450.0}"#;
    let backend = MockLlmBackend::with_canned_response(canned);

    let schema = schemars::schema_for!(SimpleRecord);
    let result = backend
        .generate_constrained("System prompt", "User document", &schema)
        .await
        .expect("generation succeeds");

    assert_eq!(result, canned);

    let parsed: SimpleRecord = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed.id, 1);
    assert_eq!(parsed.name, "Acme Invoice");
    assert_eq!(parsed.total, 450.0);
}

#[tokio::test]
async fn test_logits_processor_called_per_token() {
    let backend = MockLlmBackend::new();
    let schema = schemars::schema_for!(SimpleRecord);

    let result = backend
        .generate_constrained("Extract invoice", "Invoice text", &schema)
        .await
        .expect("generation succeeds");

    // Must have evaluated logits mask per token
    assert!(
        backend.logits_mask_evaluations() > 0,
        "Logits mask must be evaluated on every token generation step"
    );
    assert!(
        backend.tokens_generated_count() > 0,
        "Tokens must have been generated"
    );

    // Result should be valid JSON
    let val: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(val.is_object());
}

#[tokio::test]
async fn test_mock_llm_canned_invalid_schema_rejected() {
    let invalid_canned = r#"{"wrong_field": 123}"#;
    let backend = MockLlmBackend::with_canned_response(invalid_canned);

    let schema = schemars::schema_for!(SimpleRecord);
    let result = backend
        .generate_constrained("System", "User", &schema)
        .await;

    assert!(result.is_err());
}

#[test]
fn test_backend_names() {
    let mock = MockLlmBackend::new();
    assert_eq!(mock.name(), "mock-deterministic-backend");

    let bonsai = OxiBonsaiBackend::new("/models/bonsai.bin");
    assert_eq!(bonsai.name(), "oxibonsai-embedded-1bit");

    let candle = CandleBackend::new("/models/llama.gguf");
    assert_eq!(candle.name(), "candle-pure-rust-gguf");

    let tract = TractBackend::new("/models/model.onnx");
    assert_eq!(tract.name(), "tract-onnx-pure-rust");

    let llama = OxiLlamaBackend::new("/models/llama.gguf");
    assert_eq!(llama.name(), "oxillama-gguf-pure-rust");
}

#[test]
fn test_oxillama_and_oxibonsai_initialization() {
    let llama = OxiLlamaBackend::new("/models/llama3-8b.gguf")
        .with_context_tokens(2048)
        .with_temperature(0.0);
    assert_eq!(llama.name(), "oxillama-gguf-pure-rust");
    let eng_cfg = llama.to_engine_config();
    assert_eq!(eng_cfg.context_size, Some(2048));

    let bonsai = OxiBonsaiBackend::new("/models/bonsai-1bit.gguf");
    assert_eq!(bonsai.name(), "oxibonsai-embedded-1bit");
    let _parser = bonsai.stream_parser();
    let _ctrl = bonsai.create_lookahead_controller();
    let _policy = bonsai.kv_policy();
}

#[tokio::test]
async fn test_candle_backend_generation_and_device_check() {
    let canned = r#"{"id": 10, "name": "Candle Pure Rust Invoice", "total": 99.5}"#;
    let backend = CandleBackend::in_memory().with_canned_response(canned);

    assert_eq!(backend.check_device().unwrap(), "candle-cpu-pure-rust");

    let schema = schemars::schema_for!(SimpleRecord);
    let result = backend
        .generate_constrained("System", "Document text", &schema)
        .await
        .expect("Candle generation succeeds");

    assert_eq!(result, canned);
    let parsed: SimpleRecord = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed.id, 10);
    assert_eq!(parsed.name, "Candle Pure Rust Invoice");
}

#[tokio::test]
async fn test_oxillama_backend_general_generation() {
    let canned = r#"{"id": 20, "name": "OxiLLaMa GGUF Invoice", "total": 1250.0}"#;
    let backend = OxiLlamaBackend::in_memory().with_canned_response(canned);

    let schema = schemars::schema_for!(SimpleRecord);
    let result = backend
        .generate_constrained("System", "Document text", &schema)
        .await
        .expect("OxiLLaMa generation succeeds");

    assert_eq!(result, canned);
    let parsed: SimpleRecord = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed.id, 20);
    assert_eq!(parsed.name, "OxiLLaMa GGUF Invoice");
}

#[tokio::test]
async fn test_oxibonsai_backend_generation() {
    let canned = r#"{"id": 30, "name": "OxiBonsai 1-bit Invoice", "total": 15.0}"#;
    let backend = OxiBonsaiBackend::in_memory().with_canned_response(canned);

    let schema = schemars::schema_for!(SimpleRecord);
    let result = backend
        .generate_constrained("System", "Document text", &schema)
        .await
        .expect("OxiBonsai generation succeeds");

    assert_eq!(result, canned);
    let parsed: SimpleRecord = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed.id, 30);
}
