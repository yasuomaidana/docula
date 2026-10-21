use docula_core::DoculaConfig;
use docula_ocr::{LanguageAssetManager, OcrError};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_mmap_lazy_init() {
    let dir = tempdir().unwrap();
    let model_path = dir.path().join("ppocrv6_rec.bin");

    // Write dummy model weights
    let dummy_weights = b"DUMMY_PPOCR_WEIGHTS_V6_BYTES_12345678";
    {
        let mut file = std::fs::File::create(&model_path).unwrap();
        file.write_all(dummy_weights).unwrap();
    }

    let manager = LanguageAssetManager::new(dir.path());

    // Initially nothing is cached or mapped
    assert_eq!(manager.cached_asset_count(), 0);
    assert!(!manager.is_cached("ppocrv6_rec.bin"));

    // Lazy load upon first access
    let mmap1 = manager.get_or_load("ppocrv6_rec.bin").unwrap();
    assert_eq!(&mmap1[..], dummy_weights);

    // Now it should be cached
    assert_eq!(manager.cached_asset_count(), 1);
    assert!(manager.is_cached("ppocrv6_rec.bin"));

    // Second access returns the exact same cached reference
    let mmap2 = manager.get_or_load("ppocrv6_rec.bin").unwrap();
    assert_eq!(&mmap2[..], dummy_weights);
    assert_eq!(manager.cached_asset_count(), 1);
}

#[test]
fn test_custom_model_dir_override() {
    let dir = tempdir().unwrap();
    let dict_path = dir.path().join("dict.txt");
    std::fs::write(&dict_path, "token1\ntoken2\n").unwrap();

    let mut config = DoculaConfig::default();
    config.ocr.model_dir = Some(dir.path().to_path_buf());
    config.ocr.default_languages = vec!["en".to_string(), "ja".to_string()];

    let manager = LanguageAssetManager::from_config(&config);
    assert_eq!(manager.base_dir(), dir.path());
    assert_eq!(manager.languages(), &["en", "ja"]);

    let mapped = manager.get_or_load("dict.txt").unwrap();
    assert_eq!(&mapped[..], b"token1\ntoken2\n");
}

#[test]
fn test_custom_model_dir_fallback() {
    let dir = tempdir().unwrap();
    let manager = LanguageAssetManager::new(dir.path());

    let err = manager.get_or_load("non_existent_model.bin").unwrap_err();
    match err {
        OcrError::ModelNotFound { path, reason } => {
            assert!(path.ends_with("non_existent_model.bin"));
            assert!(reason.contains("does not exist"));
        }
        other => panic!("expected ModelNotFound, got: {other:?}"),
    }
}

#[test]
fn test_multilingual_feature_gating() {
    let manager = LanguageAssetManager::new("/tmp");
    // Under default features ("ocr-latin"), ocr-multilingual is false
    #[cfg(not(feature = "ocr-multilingual"))]
    assert!(!manager.is_multilingual_compiled());

    #[cfg(feature = "ocr-multilingual")]
    assert!(manager.is_multilingual_compiled());
}
