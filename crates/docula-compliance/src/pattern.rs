use crate::directive::ActionDirective;
use crate::error::ComplianceError;
use regex::Regex;

/// Deterministic regex pattern with associated action directive for automatic PII redaction.
#[derive(Clone, Debug)]
pub struct CensorPattern {
    /// Human-readable pattern name (e.g. "iban", "ssn").
    pub name: String,
    /// Compiled regular expression.
    pub regex: Regex,
    /// Target action directive to execute upon match.
    pub action: ActionDirective,
}

impl CensorPattern {
    /// Constructs a new validated `CensorPattern`.
    pub fn new(
        name: impl Into<String>,
        pattern: &str,
        action: ActionDirective,
    ) -> Result<Self, ComplianceError> {
        let regex =
            Regex::new(pattern).map_err(|e| ComplianceError::InvalidPattern(e.to_string()))?;
        Ok(Self {
            name: name.into(),
            regex,
            action,
        })
    }

    /// Evaluates if the candidate text matches this pattern.
    #[must_use]
    pub fn is_match(&self, text: &str) -> bool {
        self.regex.is_match(text)
    }

    /// Standard international bank account number (IBAN) pattern.
    #[must_use]
    pub fn default_iban() -> Self {
        Self::new(
            "iban",
            r"^[A-Z]{2}\d{2}[A-Z0-9]{4}\d{7}([A-Z0-9]?){0,16}$",
            ActionDirective::Censor,
        )
        .expect("static regex is valid")
    }

    /// Standard credit card number pattern (13-16 digits with optional dashes or spaces).
    #[must_use]
    pub fn default_credit_card() -> Self {
        Self::new(
            "credit_card",
            r"\b(?:\d[ -]*?){13,16}\b",
            ActionDirective::Censor,
        )
        .expect("static regex is valid")
    }

    /// Standard US Social Security Number pattern (`XXX-XX-XXXX`).
    #[must_use]
    pub fn default_ssn() -> Self {
        Self::new("ssn", r"\b\d{3}-\d{2}-\d{4}\b", ActionDirective::Censor)
            .expect("static regex is valid")
    }
}
