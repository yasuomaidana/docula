use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Field-level annotation directives parsed from JSON schema descriptions or doc comments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FieldDirective {
    /// Binds a visual sub-image crop of the field's spatial bounding box.
    Crop,
    /// Binds a full-page raster screenshot.
    Screenshot,
    /// Binds decoded optical code payload (QR / 1D Barcode).
    Decode,
    /// Permanently blacks out pixels and purges text tokens.
    Censor,
    /// Omits field from LLM context prompt.
    Ignore,
    /// In-memory validation only, zeroized on drop, omitted from storage.
    NonStoring,
}

impl FieldDirective {
    /// String identifier with leading `@`.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Crop => "@crop",
            Self::Screenshot => "@screenshot",
            Self::Decode => "@decode",
            Self::Censor => "@censor",
            Self::Ignore => "@ignore",
            Self::NonStoring => "@non-storing",
        }
    }

    /// Extracts all directives present in a text docstring or description.
    #[must_use]
    pub fn parse_directives(description: &str) -> Vec<Self> {
        let mut directives = Vec::new();
        for word in description.split_whitespace() {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '@' && c != '-');
            match clean.to_ascii_lowercase().as_str() {
                "@crop" => directives.push(Self::Crop),
                "@screenshot" => directives.push(Self::Screenshot),
                "@decode" => directives.push(Self::Decode),
                "@censor" => directives.push(Self::Censor),
                "@ignore" => directives.push(Self::Ignore),
                "@non-storing" | "@nonstoring" => directives.push(Self::NonStoring),
                _ => {}
            }
        }
        directives
    }
}

/// Registry mapping struct field paths to their active field directives.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DirectiveRegistry {
    entries: HashMap<String, Vec<FieldDirective>>,
}

impl DirectiveRegistry {
    /// Constructs an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Registers directives for a specific field path.
    pub fn register(&mut self, field_path: impl Into<String>, directives: Vec<FieldDirective>) {
        if !directives.is_empty() {
            self.entries.insert(field_path.into(), directives);
        }
    }

    /// Retrieves all directives associated with a given field path.
    #[must_use]
    pub fn get_directives(&self, field_path: &str) -> &[FieldDirective] {
        self.entries
            .get(field_path)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Returns `true` if the specified field path has the given directive.
    #[must_use]
    pub fn has_directive(&self, field_path: &str, directive: &FieldDirective) -> bool {
        self.get_directives(field_path).contains(directive)
    }

    /// Inspects a `serde_json::Value` JSON Schema AST and extracts all field directives.
    #[must_use]
    pub fn from_json_schema(schema: &serde_json::Value) -> Self {
        let mut registry = Self::new();
        extract_directives_recursive(schema, "", &mut registry);
        registry
    }
}

fn extract_directives_recursive(
    schema: &serde_json::Value,
    prefix: &str,
    registry: &mut DirectiveRegistry,
) {
    if let Some(desc) = schema.get("description").and_then(|d| d.as_str()) {
        let dirs = FieldDirective::parse_directives(desc);
        if !dirs.is_empty() && !prefix.is_empty() {
            registry.register(prefix, dirs);
        }
    }

    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        for (prop_name, prop_val) in props {
            let path = if prefix.is_empty() {
                prop_name.clone()
            } else {
                format!("{prefix}.{prop_name}")
            };
            extract_directives_recursive(prop_val, &path, registry);
        }
    }

    if let Some(items) = schema.get("items") {
        let path = format!("{prefix}[]");
        extract_directives_recursive(items, &path, registry);
    }
}
