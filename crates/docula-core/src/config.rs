//! Consumer-driven configuration model (`DOC-102`).
//!
//! Implements the **Builder** and **Layered Configuration** patterns specified in
//! `docs/15_CONSUMER_CONFIGURATION_AND_TOML.md`:
//!
//! 1. **Layer 0 – In-memory defaults**: [`DoculaConfig::default()`] performs zero filesystem
//!    or environment access. Storage directories default to `None` (ephemeral in-memory mode).
//! 2. **Layer 1 – Consumer TOML** (feature `toml`): [`DoculaConfig::from_toml_str`] and
//!    [`DoculaConfig::from_file`] overlay partial documents on top of the defaults; every
//!    missing field falls back to its field-level default.
//! 3. **Layer 2 – Programmatic overrides**: [`DoculaConfigBuilder`] (obtained via
//!    [`DoculaConfig::builder`] or [`DoculaConfig::into_builder`]) applies fluent overrides
//!    and validates the final configuration.
//!
//! `DoculaConfig` also implements `serde::Deserialize`, so consumers can embed it directly in
//! their own application settings structs.

use crate::error::ConfigError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Root configuration for the `docula` pipeline.
///
/// # Examples
///
/// ```
/// use docula_core::DoculaConfig;
///
/// let config = DoculaConfig::builder()
///     .with_triage_confidence(0.92)
///     .with_exemplars_enabled(false)
///     .build()
///     .expect("valid configuration");
///
/// assert_eq!(config.hitl.min_token_confidence, 0.92);
/// assert!(!config.exemplars.enabled);
/// assert!(config.is_ephemeral());
/// ```
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DoculaConfig {
    /// Engine identity and threading settings.
    pub general: GeneralConfig,
    /// Few-shot exemplar retrieval settings.
    pub exemplars: ExemplarConfig,
    /// Human-in-the-loop triage and promotion settings.
    pub hitl: HitlConfig,
    /// Edge compliance and non-storing settings.
    pub compliance: ComplianceConfig,
    /// OCR engine settings.
    pub ocr: OcrConfig,
}

/// Engine identity and threading settings (`[general]`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GeneralConfig {
    /// Human-readable engine name.
    pub engine_name: String,
    /// Configuration schema / engine version string.
    pub version: String,
    /// Upper bound for worker threads. `None` lets the runtime decide.
    pub max_threads: Option<usize>,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            engine_name: "docula".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            max_threads: None,
        }
    }
}

/// Few-shot exemplar retrieval settings (`[exemplars]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExemplarConfig {
    /// Enables few-shot exemplar injection.
    pub enabled: bool,
    /// Curated exemplar pool directory. `None` means ephemeral in-memory mode.
    pub pool_directory: Option<PathBuf>,
    /// Maximum number of exemplars injected per prompt (`1..=MAX_INJECTED_EXEMPLARS`).
    pub max_injected_exemplars: usize,
    /// Minimum cosine similarity for Stage 2 fallback, within `[0.0, 1.0]`.
    pub similarity_threshold: f32,
    /// Enables Stage 1 deterministic fingerprint matching.
    pub stage1_fast_match: bool,
    /// Enables Stage 2 HNSW vector fallback.
    pub stage2_hnsw_fallback: bool,
}

impl ExemplarConfig {
    /// Hard upper bound on injected exemplars to prevent prompt bloat.
    pub const MAX_INJECTED_EXEMPLARS: usize = 8;
}

impl Default for ExemplarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pool_directory: None,
            max_injected_exemplars: 2,
            similarity_threshold: 0.82,
            stage1_fast_match: true,
            stage2_hnsw_fallback: true,
        }
    }
}

/// Human-in-the-loop triage and promotion settings (`[hitl]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HitlConfig {
    /// Enables the active-learning flywheel.
    pub enabled: bool,
    /// Append-only cold archive directory. `None` means ephemeral in-memory mode.
    pub cold_archive_directory: Option<PathBuf>,
    /// Minimum token confidence before triage is triggered, within `[0.0, 1.0]`.
    pub min_token_confidence: f32,
    /// Minimum recurrence count before a correction may be promoted (`>= 1`).
    pub min_recurrence_for_promotion: usize,
    /// Promote corrections without explicit human review.
    pub auto_promote: bool,
}

impl Default for HitlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cold_archive_directory: None,
            min_token_confidence: 0.85,
            min_recurrence_for_promotion: 3,
            auto_promote: false,
        }
    }
}

/// Default action applied to optical codes lacking explicit schema annotations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpticalCodeAction {
    /// Decode the payload and expose it to extraction.
    #[default]
    Decode,
    /// Mask the payload text while keeping the region visible.
    Mask,
    /// Black out the region at pixel level.
    Censor,
    /// Drop the detection entirely.
    Ignore,
}

/// Edge compliance and non-storing settings (`[compliance]`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ComplianceConfig {
    /// Forbid any network egress of document data.
    pub zero_data_egress: bool,
    /// Seconds after which retained pixel buffers are discarded.
    pub ephemeral_image_ttl_seconds: u64,
    /// Default action for unannotated QR codes.
    pub unannotated_qr_code: OpticalCodeAction,
    /// Default action for unannotated 1D barcodes.
    pub unannotated_1d_barcode: OpticalCodeAction,
    /// Enables deterministic PII pre-sanitization before LLM inference.
    pub auto_censor_pii: bool,
    /// Exclude `NonStored<T>` fields from persistent serialization.
    pub strip_non_storing_fields: bool,
}

impl Default for ComplianceConfig {
    fn default() -> Self {
        Self {
            zero_data_egress: true,
            ephemeral_image_ttl_seconds: 86_400,
            unannotated_qr_code: OpticalCodeAction::Decode,
            unannotated_1d_barcode: OpticalCodeAction::Mask,
            auto_censor_pii: true,
            strip_non_storing_fields: true,
        }
    }
}

/// OCR engine settings (`[ocr]`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OcrConfig {
    /// Custom model weights directory. `None` uses bundled / default assets.
    pub model_dir: Option<PathBuf>,
    /// Rasterization DPI used for OCR (`MIN_DPI..=MAX_DPI`).
    pub dpi: u32,
    /// Default recognition languages (ISO 639-1 codes). Must not be empty.
    pub default_languages: Vec<String>,
}

impl OcrConfig {
    /// Minimum supported rasterization DPI.
    pub const MIN_DPI: u32 = 72;
    /// Maximum supported rasterization DPI.
    pub const MAX_DPI: u32 = 1200;
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            model_dir: None,
            dpi: 300,
            default_languages: vec!["en".to_owned()],
        }
    }
}

impl DoculaConfig {
    /// Starts a fluent builder seeded with in-memory defaults.
    #[inline]
    pub fn builder() -> DoculaConfigBuilder {
        DoculaConfigBuilder::default()
    }

    /// Converts this configuration into a builder for layering programmatic overrides.
    #[inline]
    pub fn into_builder(self) -> DoculaConfigBuilder {
        DoculaConfigBuilder { config: self }
    }

    /// Returns `true` when no persistent storage directories are configured, meaning
    /// the pipeline must operate in ephemeral in-memory mode.
    #[inline]
    pub fn is_ephemeral(&self) -> bool {
        self.exemplars.pool_directory.is_none() && self.hitl.cold_archive_directory.is_none()
    }

    /// Validates all documented invariants.
    ///
    /// # Errors
    /// Returns [`ConfigError::InvalidValue`] for the first violated invariant.
    pub fn validate(&self) -> Result<(), ConfigError> {
        validate_unit_interval(
            "exemplars.similarity_threshold",
            self.exemplars.similarity_threshold,
        )?;
        validate_unit_interval("hitl.min_token_confidence", self.hitl.min_token_confidence)?;
        self.validate_counts()?;
        self.validate_ocr()
    }

    fn validate_counts(&self) -> Result<(), ConfigError> {
        let injected = self.exemplars.max_injected_exemplars;
        if !(1..=ExemplarConfig::MAX_INJECTED_EXEMPLARS).contains(&injected) {
            return Err(invalid(
                "exemplars.max_injected_exemplars",
                format!(
                    "must be within 1..={} (got {injected})",
                    ExemplarConfig::MAX_INJECTED_EXEMPLARS
                ),
            ));
        }
        if self.hitl.min_recurrence_for_promotion == 0 {
            return Err(invalid(
                "hitl.min_recurrence_for_promotion",
                "must be >= 1".to_owned(),
            ));
        }
        if self.general.max_threads == Some(0) {
            return Err(invalid(
                "general.max_threads",
                "must be >= 1 when set".to_owned(),
            ));
        }
        Ok(())
    }

    fn validate_ocr(&self) -> Result<(), ConfigError> {
        let dpi = self.ocr.dpi;
        if !(OcrConfig::MIN_DPI..=OcrConfig::MAX_DPI).contains(&dpi) {
            return Err(invalid(
                "ocr.dpi",
                format!(
                    "must be within {}..={} (got {dpi})",
                    OcrConfig::MIN_DPI,
                    OcrConfig::MAX_DPI
                ),
            ));
        }
        if self.ocr.default_languages.is_empty() {
            return Err(invalid(
                "ocr.default_languages",
                "must not be empty".to_owned(),
            ));
        }
        if self
            .ocr
            .default_languages
            .iter()
            .any(|l| l.trim().is_empty())
        {
            return Err(invalid(
                "ocr.default_languages",
                "must not contain blank language codes".to_owned(),
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "toml")]
impl DoculaConfig {
    /// Parses a (possibly partial) TOML document layered over in-memory defaults and
    /// validates the result.
    ///
    /// # Errors
    /// - [`ConfigError::ParseFailure`] on malformed TOML or unknown / mistyped keys.
    /// - [`ConfigError::InvalidValue`] when a parsed value violates an invariant.
    ///
    /// # Examples
    /// ```
    /// use docula_core::DoculaConfig;
    ///
    /// let config = DoculaConfig::from_toml_str("[ocr]\ndpi = 150\n").expect("valid toml");
    /// assert_eq!(config.ocr.dpi, 150);
    /// assert_eq!(config.hitl.min_token_confidence, 0.85); // default preserved
    /// ```
    pub fn from_toml_str(toml_str: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(toml_str).map_err(|e| ConfigError::ParseFailure {
            reason: e.to_string(),
        })?;
        config.validate()?;
        Ok(config)
    }

    /// Loads configuration from an arbitrary consumer-specified TOML file path.
    ///
    /// # Errors
    /// - [`ConfigError::FileRead`] when the file cannot be read.
    /// - Any error produced by [`DoculaConfig::from_toml_str`].
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::FileRead {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_toml_str(&contents)
    }

    /// Serializes the configuration into a TOML document.
    ///
    /// # Errors
    /// Returns [`ConfigError::SerializeFailure`] if TOML encoding fails.
    pub fn to_toml_string(&self) -> Result<String, ConfigError> {
        toml::to_string(self).map_err(|e| ConfigError::SerializeFailure {
            reason: e.to_string(),
        })
    }
}

/// Fluent builder applying programmatic overrides on top of a base configuration.
#[derive(Clone, Debug, Default)]
#[must_use = "a builder does nothing until `build()` is called"]
pub struct DoculaConfigBuilder {
    config: DoculaConfig,
}

impl DoculaConfigBuilder {
    /// Sets the HITL triage token-confidence threshold.
    pub fn with_triage_confidence(mut self, threshold: f32) -> Self {
        self.config.hitl.min_token_confidence = threshold;
        self
    }

    /// Enables or disables few-shot exemplar injection (pure zero-shot when `false`).
    pub fn with_exemplars_enabled(mut self, enabled: bool) -> Self {
        self.config.exemplars.enabled = enabled;
        self
    }

    /// Sets the curated exemplar pool directory.
    pub fn with_pool_directory(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.exemplars.pool_directory = Some(dir.into());
        self
    }

    /// Sets the maximum number of injected exemplars.
    pub fn with_max_injected_exemplars(mut self, count: usize) -> Self {
        self.config.exemplars.max_injected_exemplars = count;
        self
    }

    /// Sets the Stage 2 cosine-similarity threshold.
    pub fn with_similarity_threshold(mut self, threshold: f32) -> Self {
        self.config.exemplars.similarity_threshold = threshold;
        self
    }

    /// Sets the HITL cold archive directory.
    pub fn with_cold_archive_directory(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.hitl.cold_archive_directory = Some(dir.into());
        self
    }

    /// Enables or disables the HITL flywheel.
    pub fn with_hitl_enabled(mut self, enabled: bool) -> Self {
        self.config.hitl.enabled = enabled;
        self
    }

    /// When `true`, clears every persistent storage directory so the pipeline runs in
    /// ephemeral in-memory mode. `false` leaves configured directories untouched.
    pub fn with_ephemeral_images(mut self, ephemeral: bool) -> Self {
        if ephemeral {
            self.config.exemplars.pool_directory = None;
            self.config.hitl.cold_archive_directory = None;
        }
        self
    }

    /// Sets the retention window for ephemeral pixel buffers.
    pub fn with_ephemeral_image_ttl_seconds(mut self, seconds: u64) -> Self {
        self.config.compliance.ephemeral_image_ttl_seconds = seconds;
        self
    }

    /// Enables or disables deterministic PII auto-censoring.
    pub fn with_auto_censor_pii(mut self, enabled: bool) -> Self {
        self.config.compliance.auto_censor_pii = enabled;
        self
    }

    /// Sets the custom OCR model directory.
    pub fn with_ocr_model_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.ocr.model_dir = Some(dir.into());
        self
    }

    /// Sets the OCR rasterization DPI.
    pub fn with_ocr_dpi(mut self, dpi: u32) -> Self {
        self.config.ocr.dpi = dpi;
        self
    }

    /// Sets the default OCR languages.
    pub fn with_ocr_languages<I, S>(mut self, languages: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.config.ocr.default_languages = languages.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the worker-thread upper bound.
    pub fn with_max_threads(mut self, threads: usize) -> Self {
        self.config.general.max_threads = Some(threads);
        self
    }

    /// Validates and returns the final configuration.
    ///
    /// # Errors
    /// Returns [`ConfigError::InvalidValue`] if any override violates an invariant.
    pub fn build(self) -> Result<DoculaConfig, ConfigError> {
        self.config.validate()?;
        Ok(self.config)
    }
}

fn validate_unit_interval(field: &'static str, value: f32) -> Result<(), ConfigError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(invalid(
            field,
            format!("must be within [0.0, 1.0] (got {value})"),
        ))
    }
}

fn invalid(field: &'static str, reason: String) -> ConfigError {
    ConfigError::InvalidValue { field, reason }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_is_valid() {
        let config = DoculaConfig::default();
        assert!(config.validate().is_ok());
        assert!(config.is_ephemeral());
        assert_eq!(config.general.engine_name, "docula");
        assert_eq!(config.exemplars.max_injected_exemplars, 2);
        assert_eq!(config.hitl.min_recurrence_for_promotion, 3);
        assert!(!config.hitl.auto_promote);
        assert_eq!(
            config.compliance.unannotated_1d_barcode,
            OpticalCodeAction::Mask
        );
        assert_eq!(config.ocr.dpi, 300);
    }

    #[test]
    fn test_unit_interval_rejects_nan_and_out_of_range() {
        assert!(validate_unit_interval("f", f32::NAN).is_err());
        assert!(validate_unit_interval("f", -0.01).is_err());
        assert!(validate_unit_interval("f", 1.01).is_err());
        assert!(validate_unit_interval("f", 0.0).is_ok());
        assert!(validate_unit_interval("f", 1.0).is_ok());
    }
}
