use crate::error::OcrError;
use docula_core::DoculaConfig;
use memmap2::Mmap;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Thread-safe zero-copy memory-mapped asset manager for OCR model weights and dictionaries.
#[derive(Debug)]
pub struct LanguageAssetManager {
    base_dir: PathBuf,
    languages: Vec<String>,
    model_cache: RwLock<HashMap<String, Arc<Mmap>>>,
}

impl LanguageAssetManager {
    /// Creates a new `LanguageAssetManager` with a target base directory.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
            languages: vec!["en".to_string()],
            model_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Creates a new `LanguageAssetManager` with explicit active languages.
    pub fn with_languages(base_dir: impl Into<PathBuf>, languages: Vec<String>) -> Self {
        Self {
            base_dir: base_dir.into(),
            languages,
            model_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Initializes asset manager from global `DoculaConfig`.
    pub fn from_config(config: &DoculaConfig) -> Self {
        let base_dir = config
            .ocr
            .model_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("./models"));
        Self {
            base_dir,
            languages: config.ocr.default_languages.clone(),
            model_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Base model directory.
    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// List of configured languages.
    #[must_use]
    pub fn languages(&self) -> &[String] {
        &self.languages
    }

    /// Checks if an asset is already mapped in memory.
    #[must_use]
    pub fn is_cached(&self, asset_name: &str) -> bool {
        self.model_cache
            .read()
            .map(|cache| cache.contains_key(asset_name))
            .unwrap_or(false)
    }

    /// Lazily loads and memory-maps the specified asset if not already cached.
    pub fn get_or_load(&self, asset_name: &str) -> Result<Arc<Mmap>, OcrError> {
        // Fast path: check read lock
        if let Some(mmap) = self
            .model_cache
            .read()
            .ok()
            .and_then(|c| c.get(asset_name).cloned())
        {
            return Ok(mmap);
        }

        // Slow path: acquire write lock and map file
        let mut cache = self.model_cache.write().map_err(|_| {
            OcrError::InferenceFailure("Failed to acquire model cache write lock".to_string())
        })?;

        // Double check pattern
        if let Some(mmap) = cache.get(asset_name) {
            return Ok(Arc::clone(mmap));
        }

        let asset_path = self.base_dir.join(asset_name);
        if !asset_path.exists() {
            return Err(OcrError::ModelNotFound {
                path: asset_path,
                reason: "Model asset file does not exist on filesystem".to_string(),
            });
        }

        let file = File::open(&asset_path).map_err(|e| OcrError::ModelNotFound {
            path: asset_path.clone(),
            reason: format!("Failed to open model asset file: {e}"),
        })?;

        // SAFETY: We map the file in read-only mode and do not modify underlying contents.
        let mmap = unsafe {
            Mmap::map(&file).map_err(|e| OcrError::MmapError {
                path: asset_path.clone(),
                reason: format!("Failed to memory-map asset: {e}"),
            })?
        };

        let arc_mmap = Arc::new(mmap);
        cache.insert(asset_name.to_string(), Arc::clone(&arc_mmap));
        Ok(arc_mmap)
    }

    /// Returns whether multi-lingual feature is compiled and enabled.
    #[must_use]
    pub const fn is_multilingual_compiled(&self) -> bool {
        cfg!(feature = "ocr-multilingual")
    }

    /// Number of actively cached mmap assets.
    #[must_use]
    pub fn cached_asset_count(&self) -> usize {
        self.model_cache
            .read()
            .map(|cache| cache.len())
            .unwrap_or(0)
    }
}
