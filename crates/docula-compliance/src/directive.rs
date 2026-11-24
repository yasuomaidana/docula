use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Action directive governing privacy, redaction, and storage behaviors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActionDirective {
    /// Spatial Blackout & Token Purge: Permanently overwrites pixels with `[0, 0, 0, 255]`
    /// and purges intersecting text spans from layout map.
    Censor,
    /// Prompt Omission: Excludes matching spatial regions and text spans from LLM context.
    Ignore,
    /// Zero Persistence: In-memory validation only, zeroized on drop and omitted from storage.
    NonStoring,
}

impl ActionDirective {
    /// String identifier of the action directive.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Censor => "censor",
            Self::Ignore => "ignore",
            Self::NonStoring => "non-storing",
        }
    }
}

impl fmt::Display for ActionDirective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ActionDirective {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "censor" | "@censor" => Ok(Self::Censor),
            "ignore" | "@ignore" => Ok(Self::Ignore),
            "non-storing" | "@non-storing" | "nonstoring" => Ok(Self::NonStoring),
            other => Err(format!("Unknown action directive: '{other}'")),
        }
    }
}
