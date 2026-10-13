use serde::{Deserialize, Serialize};

/// Optional integrity validation trait for domain-specific consistency checks.
///
/// If implemented, Docula runs integrity validation after deserialization and
/// automatically triggers targeted self-repair loops if validation errors are detected.
pub trait DocuValidate {
    /// Validates business rules, arithmetic consistency, or internal formatting invariants.
    fn validate_integrity(&self) -> Result<(), Vec<ValidationError>>;
}

/// Strongly typed integrity validation failure details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationError {
    /// Dot-separated field path (e.g. `items[0].total` or `financials.tax`).
    pub field_path: String,
    /// Human-readable explanation of why validation failed.
    pub error_message: String,
}

impl ValidationError {
    /// Constructs a new `ValidationError`.
    pub fn new(field_path: impl Into<String>, error_message: impl Into<String>) -> Self {
        Self {
            field_path: field_path.into(),
            error_message: error_message.into(),
        }
    }
}

// =========================================================================
// Composable Validation Assertion Engine (CC <= 4, Cognitive <= 4)
// =========================================================================

/// Composable assertion builder for zero-boilerplate domain validation.
///
/// Enables expressive, readable assertions while keeping cyclomatic and cognitive
/// complexity well below audit thresholds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Validator {
    errors: Vec<ValidationError>,
}

impl Validator {
    /// Constructs a new, clean validation collector.
    #[must_use]
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Evaluates a general boolean assertion condition.
    pub fn assert(
        &mut self,
        condition: bool,
        field_path: impl Into<String>,
        error_message: impl Into<String>,
    ) -> &mut Self {
        if !condition {
            self.errors
                .push(ValidationError::new(field_path, error_message));
        }
        self
    }

    /// Asserts that two floating-point values are approximately equal within a specified tolerance.
    pub fn assert_approx_eq(
        &mut self,
        calculated: f64,
        expected: f64,
        tolerance: f64,
        field_path: impl Into<String>,
        error_message: impl Into<String>,
    ) -> &mut Self {
        if (calculated - expected).abs() > tolerance {
            self.errors
                .push(ValidationError::new(field_path, error_message));
        }
        self
    }

    /// Evaluates a list of fallback candidate rules `(rule_name, is_valid)`.
    ///
    /// If ANY candidate is satisfied, the assertion passes.
    /// If ALL candidate conditions fail, records an error detailing the evaluated options.
    ///
    /// Example: Total in a bill can match `subtotal - discount`, or `subtotal - tax`, or `subtotal`.
    pub fn assert_fallback(
        &mut self,
        field_path: impl Into<String>,
        candidates: &[(&str, bool)],
        error_message: impl Into<String>,
    ) -> &mut Self {
        let any_passed = candidates.iter().any(|(_, pass)| *pass);
        if !any_passed {
            let names: Vec<&str> = candidates.iter().map(|(name, _)| *name).collect();
            let msg = error_message.into();
            self.errors.push(ValidationError::new(
                field_path,
                format!(
                    "{msg} (evaluated fallback candidates: {})",
                    names.join(", ")
                ),
            ));
        }
        self
    }

    /// Asserts that a date string (YYYY-MM-DD) contains a year within the range `[min_year, max_year]`.
    pub fn assert_year_between(
        &mut self,
        field_path: impl Into<String>,
        date_str: &str,
        min_year: i32,
        max_year: i32,
    ) -> &mut Self {
        let clean = date_str.trim();
        let valid_year = if clean.len() >= 4
            && let Ok(year) = clean[..4].parse::<i32>()
        {
            year >= min_year && year <= max_year
        } else {
            false
        };

        if !valid_year {
            self.errors.push(ValidationError::new(
                field_path,
                format!("Date {clean} year must be between {min_year} and {max_year}"),
            ));
        }
        self
    }

    /// Asserts that a string is non-empty after whitespace trimming.
    pub fn assert_non_empty(
        &mut self,
        field_path: impl Into<String>,
        value: &str,
        entity_name: &str,
    ) -> &mut Self {
        if value.trim().is_empty() {
            self.errors.push(ValidationError::new(
                field_path,
                format!("{entity_name} must not be empty"),
            ));
        }
        self
    }

    /// Directly records a validation error.
    pub fn add_error(&mut self, error: ValidationError) -> &mut Self {
        self.errors.push(error);
        self
    }

    /// Appends multiple validation errors.
    pub fn extend_errors(&mut self, errors: Vec<ValidationError>) -> &mut Self {
        self.errors.extend(errors);
        self
    }

    /// Consumes the validator, returning `Ok(())` if zero errors, or `Err(Vec<ValidationError>)`.
    pub fn finish(self) -> Result<(), Vec<ValidationError>> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }
}

// =========================================================================
// Validation Outcome Modes (Strict, Soft, Error)
// =========================================================================

/// Operating policy dictating how validation failures are handled during extraction.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationMode {
    /// Strict mode: Extractions that fail domain validation are rejected and routed to triage.
    #[default]
    Strict,
    /// Soft mode: Extractions that parse into the target struct are accepted as Soft outcomes with warnings.
    Soft,
    /// Permissive mode: All extractions that parse into the target struct are accepted without triage routing.
    Permissive,
}

/// Strongly typed result set distinguishing strict passes, soft passes, and syntax/schema errors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "payload")]
pub enum ValidationOutcome<T> {
    /// Extraction parsed into target struct T and passed all domain integrity checks.
    #[serde(rename = "strict")]
    Strict(T),

    /// Extraction successfully parsed into target struct T, but one or more domain validations failed.
    #[serde(rename = "soft")]
    Soft {
        /// Parsed domain data.
        data: T,
        /// List of domain validation warnings/failures.
        validation_errors: Vec<ValidationError>,
    },

    /// Extraction completely failed to produce a valid schema instance (e.g. JSON syntax or schema failure).
    #[serde(rename = "error")]
    Error {
        /// Raw output generated by the model.
        raw_output: String,
        /// Explanation of parse or schema failure.
        message: String,
    },
}

impl<T: DocuValidate> ValidationOutcome<T> {
    /// Evaluates parsed domain entity `data` using its `DocuValidate` implementation.
    ///
    /// Returns `Strict(data)` if all validations pass, or `Soft { data, validation_errors }` otherwise.
    pub fn from_parsed(data: T) -> Self {
        match data.validate_integrity() {
            Ok(()) => Self::Strict(data),
            Err(validation_errors) => Self::Soft {
                data,
                validation_errors,
            },
        }
    }
}

impl<T> ValidationOutcome<T> {
    /// Returns true if the extraction is strictly valid.
    #[must_use]
    pub fn is_strict(&self) -> bool {
        matches!(self, Self::Strict(_))
    }

    /// Returns true if the extraction parsed into T with soft validation warnings.
    #[must_use]
    pub fn is_soft(&self) -> bool {
        matches!(self, Self::Soft { .. })
    }

    /// Returns true if the extraction failed completely.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Returns a reference to the parsed domain entity if available (Strict or Soft).
    #[must_use]
    pub fn data(&self) -> Option<&T> {
        match self {
            Self::Strict(data) | Self::Soft { data, .. } => Some(data),
            Self::Error { .. } => None,
        }
    }

    /// Consumes the outcome, yielding the parsed domain entity if available.
    #[must_use]
    pub fn into_data(self) -> Option<T> {
        match self {
            Self::Strict(data) | Self::Soft { data, .. } => Some(data),
            Self::Error { .. } => None,
        }
    }

    /// Returns domain validation errors if present.
    #[must_use]
    pub fn validation_errors(&self) -> &[ValidationError] {
        match self {
            Self::Soft {
                validation_errors, ..
            } => validation_errors,
            _ => &[],
        }
    }
}

// =========================================================================
// Declarative Macro: docula_model!
// =========================================================================

/// Declarative macro for defining custom document models with arbitrary fields
/// and ergonomic, modular validation assertions.
#[macro_export]
macro_rules! docula_model {
    // Branch with custom validation block
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_ty:ty
            ),* $(,)?
        }
        validate($self:ident, $v:ident) $val_body:block
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema)]
        $vis struct $name {
            $(
                $(#[$field_meta])*
                $field_vis $field_name : $field_ty,
            )*
        }

        impl $crate::DocuValidate for $name {
            fn validate_integrity(&$self) -> Result<(), Vec<$crate::ValidationError>> {
                let mut $v = $crate::Validator::new();
                $val_body
                $v.finish()
            }
        }
    };

    // Branch without validation block (pure schema model)
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_ty:ty
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema)]
        $vis struct $name {
            $(
                $(#[$field_meta])*
                $field_vis $field_name : $field_ty,
            )*
        }

        impl $crate::DocuValidate for $name {
            fn validate_integrity(&self) -> Result<(), Vec<$crate::ValidationError>> {
                Ok(())
            }
        }
    };
}
