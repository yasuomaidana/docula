//! Integration tests for DOC-102: Consumer-Driven `DoculaConfig` & Layered Loader.
//!
//! Traceability: `docs/01_DEVELOPMENT_PLAN.md` (DOC-102) and
//! `docs/15_CONSUMER_CONFIGURATION_AND_TOML.md` §4.

use docula_core::{ConfigError, DoculaConfig, DoculaError, OpticalCodeAction};
use serde::Deserialize;
use std::path::PathBuf;

// AC1 / spec 15 `test_default_config_is_pure_in_memory`
#[test]
fn test_config_default_is_valid() {
    let config = DoculaConfig::default();
    assert!(config.validate().is_ok());
    assert!(
        config.is_ephemeral(),
        "defaults must not reference any directory"
    );
    assert_eq!(config.exemplars.pool_directory, None);
    assert_eq!(config.hitl.cold_archive_directory, None);
    assert_eq!(config.ocr.model_dir, None);
    assert!((config.hitl.min_token_confidence - 0.85).abs() < f32::EPSILON);
    assert!((config.exemplars.similarity_threshold - 0.82).abs() < f32::EPSILON);
    assert!(config.compliance.zero_data_egress);
    assert_eq!(
        config.compliance.unannotated_qr_code,
        OpticalCodeAction::Decode
    );
}

// AC4 / `test_config_builder_overrides`
#[test]
fn test_config_builder_overrides() {
    let config = DoculaConfig::builder()
        .with_pool_directory("/var/data/docula/examples")
        .with_triage_confidence(0.90)
        .with_exemplars_enabled(false)
        .with_ocr_languages(["en", "es"])
        .build()
        .expect("valid overrides");

    assert_eq!(
        config.exemplars.pool_directory,
        Some(PathBuf::from("/var/data/docula/examples"))
    );
    assert!((config.hitl.min_token_confidence - 0.90).abs() < f32::EPSILON);
    assert!(!config.exemplars.enabled);
    assert_eq!(config.ocr.default_languages, vec!["en", "es"]);
    assert!(!config.is_ephemeral());
    // Untouched fields keep defaults.
    assert_eq!(config.ocr.dpi, 300);
}

#[test]
fn test_builder_ephemeral_clears_storage_directories() {
    let config = DoculaConfig::builder()
        .with_pool_directory("/tmp/pool")
        .with_cold_archive_directory("/tmp/archive")
        .with_ephemeral_images(true)
        .build()
        .expect("valid");
    assert!(config.is_ephemeral());
}

#[test]
fn test_builder_rejects_out_of_range_confidence() {
    let err = DoculaConfig::builder()
        .with_triage_confidence(1.5)
        .build()
        .expect_err("confidence > 1.0 must fail");
    assert!(matches!(
        err,
        ConfigError::InvalidValue {
            field: "hitl.min_token_confidence",
            ..
        }
    ));
}

#[test]
fn test_builder_rejects_boundary_violations() {
    let cases = [
        DoculaConfig::builder()
            .with_max_injected_exemplars(0)
            .build(),
        DoculaConfig::builder()
            .with_max_injected_exemplars(9)
            .build(),
        DoculaConfig::builder().with_ocr_dpi(0).build(),
        DoculaConfig::builder().with_ocr_dpi(1201).build(),
        DoculaConfig::builder()
            .with_ocr_languages(Vec::<String>::new())
            .build(),
        DoculaConfig::builder().with_ocr_languages([" "]).build(),
        DoculaConfig::builder().with_max_threads(0).build(),
        DoculaConfig::builder()
            .with_similarity_threshold(f32::NAN)
            .build(),
    ];
    for (i, result) in cases.into_iter().enumerate() {
        assert!(
            matches!(result, Err(ConfigError::InvalidValue { .. })),
            "case {i} should be rejected"
        );
    }
}

#[test]
fn test_builder_accepts_boundary_values() {
    let config = DoculaConfig::builder()
        .with_max_injected_exemplars(1)
        .with_ocr_dpi(72)
        .with_triage_confidence(0.0)
        .with_similarity_threshold(1.0)
        .build();
    assert!(config.is_ok());
}

// AC2 + spec 15 `test_from_toml_str_partial`
#[test]
fn test_from_toml_str_partial() {
    let config = DoculaConfig::from_toml_str("[compliance]\nauto_censor_pii = false\n")
        .expect("partial toml must parse");
    assert!(!config.compliance.auto_censor_pii);

    let mut expected = DoculaConfig::default();
    expected.compliance.auto_censor_pii = false;
    assert_eq!(config, expected, "all other fields must keep defaults");
}

#[test]
fn test_from_toml_str_empty_yields_defaults() {
    let config = DoculaConfig::from_toml_str("").expect("empty toml is valid");
    assert_eq!(config, DoculaConfig::default());
}

// `test_config_from_toml_str_malformed`
#[test]
fn test_config_from_toml_str_malformed() {
    let err = DoculaConfig::from_toml_str("[hitl\nmin_token_confidence = ").expect_err("malformed");
    assert!(matches!(err, ConfigError::ParseFailure { .. }));
}

#[test]
fn test_from_toml_str_unknown_key_is_rejected() {
    let err = DoculaConfig::from_toml_str("[hitl]\nmin_token_confidance = 0.9\n")
        .expect_err("typo must be rejected");
    assert!(matches!(err, ConfigError::ParseFailure { .. }));
}

#[test]
fn test_from_toml_str_wrong_type_is_rejected() {
    let err = DoculaConfig::from_toml_str("[ocr]\ndpi = \"high\"\n").expect_err("wrong type");
    assert!(matches!(err, ConfigError::ParseFailure { .. }));
}

#[test]
fn test_from_toml_str_invalid_action_is_rejected() {
    let err = DoculaConfig::from_toml_str("[compliance]\nunannotated_qr_code = \"shred\"\n")
        .expect_err("unknown action");
    assert!(matches!(err, ConfigError::ParseFailure { .. }));
}

#[test]
fn test_from_toml_str_out_of_range_is_invalid_value() {
    let err = DoculaConfig::from_toml_str("[exemplars]\nsimilarity_threshold = 2.0\n")
        .expect_err("out of range");
    assert!(matches!(
        err,
        ConfigError::InvalidValue {
            field: "exemplars.similarity_threshold",
            ..
        }
    ));
}

// AC3: arbitrary consumer path
#[test]
fn test_from_file_custom_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("production.toml");
    std::fs::write(&path, "[ocr]\ndpi = 150\nmodel_dir = \"/opt/models\"\n").expect("write");

    let config = DoculaConfig::from_file(&path).expect("load from file");
    assert_eq!(config.ocr.dpi, 150);
    assert_eq!(config.ocr.model_dir, Some(PathBuf::from("/opt/models")));
}

#[test]
fn test_from_file_missing_path_is_file_read_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("does-not-exist.toml");
    let err = DoculaConfig::from_file(&missing).expect_err("missing file");
    match err {
        ConfigError::FileRead { path, source } => {
            assert_eq!(path, missing);
            assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn test_layered_file_then_builder_overrides() {
    let base = DoculaConfig::from_toml_str("[hitl]\nmin_token_confidence = 0.70\n").expect("base");
    let config = base
        .into_builder()
        .with_triage_confidence(0.95)
        .build()
        .expect("layered");
    assert!((config.hitl.min_token_confidence - 0.95).abs() < f32::EPSILON);
}

#[test]
fn test_toml_round_trip() {
    let original = DoculaConfig::builder()
        .with_cold_archive_directory("/data/corrections")
        .with_max_threads(4)
        .build()
        .expect("valid");
    let encoded = original.to_toml_string().expect("serialize");
    let decoded = DoculaConfig::from_toml_str(&encoded).expect("deserialize");
    assert_eq!(original, decoded);
}

#[test]
fn test_repository_template_parses() {
    let config = DoculaConfig::from_toml_str(include_str!("../../../Docula.toml"))
        .expect("Docula.toml template must stay in sync with DoculaConfig");
    assert_eq!(
        config.hitl.cold_archive_directory,
        Some(PathBuf::from("./data/corrections"))
    );
    assert_eq!(
        config.compliance.unannotated_1d_barcode,
        OpticalCodeAction::Mask
    );
}

// Spec 15 `test_embed_in_consumer_struct`
#[test]
fn test_embed_in_consumer_struct() {
    #[derive(Deserialize)]
    struct AppConfig {
        server_port: u16,
        docula: DoculaConfig,
    }

    let app: AppConfig = toml::from_str(
        "server_port = 8080\n\n[docula.hitl]\nauto_promote = true\n\n[docula.ocr]\ndpi = 600\n",
    )
    .expect("consumer struct must deserialize");

    assert_eq!(app.server_port, 8080);
    assert!(app.docula.hitl.auto_promote);
    assert_eq!(app.docula.ocr.dpi, 600);
    assert_eq!(app.docula.exemplars, DoculaConfig::default().exemplars);
}

#[test]
fn test_config_error_converts_into_docula_error() {
    let err: DoculaError = ConfigError::ParseFailure {
        reason: "x".to_owned(),
    }
    .into();
    assert!(matches!(err, DoculaError::Config(_)));
}
