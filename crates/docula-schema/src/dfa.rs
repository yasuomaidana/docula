use crate::directives::DirectiveRegistry;
use crate::error::SchemaError;
use serde_json::Value;

/// Deterministic Finite Automaton (DFA) state representation during JSON generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DfaState {
    /// Initial state, expecting root opening (e.g. `{` for objects or `[` for arrays).
    Initial,
    /// Expecting object property key (quoted string).
    ExpectKey,
    /// Expecting colon separator `:` after property key.
    ExpectColon,
    /// Expecting property value according to field schema.
    ExpectValue,
    /// Expecting comma `,` or closing brace `}`.
    ExpectCommaOrClose,
    /// Fully accepted valid JSON document.
    Accepted,
    /// Dead or invalid state.
    Error,
}

/// Deterministic Finite Automaton compiler and grammar validator for JSON schemas.
#[derive(Clone, Debug)]
pub struct SchemaDfa {
    /// Parsed schema JSON AST.
    schema_json: Value,
    /// Extracted field directives registry.
    directives: DirectiveRegistry,
    /// Root type expected by the schema ("object", "array", etc.).
    root_type: String,
}

impl SchemaDfa {
    /// Compiles a `schemars::Schema` into an active `SchemaDfa`.
    pub fn compile(schema: &schemars::Schema) -> Result<Self, SchemaError> {
        let schema_json: Value = serde_json::to_value(schema).map_err(|e| {
            SchemaError::ParseError(format!("Failed to serialize schema to Value: {e}"))
        })?;
        Self::from_json_value(schema_json)
    }

    /// Compiles a raw JSON Schema string into an active `SchemaDfa`.
    pub fn compile_json(json_str: &str) -> Result<Self, SchemaError> {
        let schema_json: Value = serde_json::from_str(json_str).map_err(|e| {
            SchemaError::ParseError(format!("Failed to parse JSON Schema string: {e}"))
        })?;
        Self::from_json_value(schema_json)
    }

    /// Creates a `SchemaDfa` directly from a parsed `serde_json::Value`.
    pub fn from_json_value(schema_json: Value) -> Result<Self, SchemaError> {
        let directives = DirectiveRegistry::from_json_schema(&schema_json);
        let root_type = schema_json
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("object")
            .to_string();

        Ok(Self {
            schema_json,
            directives,
            root_type,
        })
    }

    /// Extracted field directives registry.
    #[must_use]
    pub fn directives(&self) -> &DirectiveRegistry {
        &self.directives
    }

    /// Root schema type ("object", "array", etc.).
    #[must_use]
    pub fn root_type(&self) -> &str {
        &self.root_type
    }

    /// Evaluates whether appending `next_token` to `current_prefix` yields a valid JSON prefix.
    #[must_use]
    pub fn can_transition(&self, current_prefix: &str, next_token: &str) -> bool {
        let candidate = format!("{current_prefix}{next_token}");
        self.is_valid_prefix(&candidate)
    }

    /// Checks if candidate string is a valid prefix of a JSON document adhering to this schema.
    #[must_use]
    pub fn is_valid_prefix(&self, candidate: &str) -> bool {
        let trimmed = candidate.trim_start();
        if trimmed.is_empty() {
            return true;
        }

        // Root opening check
        if self.root_type == "object" && !trimmed.starts_with('{') {
            return false;
        }
        if self.root_type == "array" && !trimmed.starts_with('[') {
            return false;
        }

        // Structural lexical scan
        validate_json_prefix_tokens(trimmed)
    }

    /// Advances the prefix with `token`, returning the new prefix if valid, or `SchemaError`.
    pub fn advance(&self, current_prefix: &str, token: &str) -> Result<String, SchemaError> {
        if self.can_transition(current_prefix, token) {
            Ok(format!("{current_prefix}{token}"))
        } else {
            Err(SchemaError::InvalidTransition(format!(
                "Token '{token}' invalid for prefix '{current_prefix}'"
            )))
        }
    }

    /// Evaluates if the candidate string is a complete and accepted JSON document.
    #[must_use]
    pub fn is_accepted(&self, candidate: &str) -> bool {
        if let Ok(val) = serde_json::from_str::<Value>(candidate) {
            validate_complete_value(&val, &self.schema_json)
        } else {
            false
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ContainerState {
    KeyOrClose,
    Colon,
    Value,
    CommaOrClose,
    ArrValueOrClose,
    ArrCommaOrClose,
}

fn handle_open_brace(stack: &mut Vec<ContainerState>) -> bool {
    if let Some(top) = stack.last_mut() {
        match top {
            ContainerState::Value => {
                *top = ContainerState::CommaOrClose;
                stack.push(ContainerState::KeyOrClose);
                true
            }
            ContainerState::ArrValueOrClose => {
                *top = ContainerState::ArrCommaOrClose;
                stack.push(ContainerState::KeyOrClose);
                true
            }
            _ => false,
        }
    } else {
        stack.push(ContainerState::KeyOrClose);
        true
    }
}

fn handle_open_bracket(stack: &mut Vec<ContainerState>) -> bool {
    if let Some(top) = stack.last_mut() {
        match top {
            ContainerState::Value => {
                *top = ContainerState::CommaOrClose;
                stack.push(ContainerState::ArrValueOrClose);
                true
            }
            ContainerState::ArrValueOrClose => {
                *top = ContainerState::ArrCommaOrClose;
                stack.push(ContainerState::ArrValueOrClose);
                true
            }
            _ => false,
        }
    } else {
        stack.push(ContainerState::ArrValueOrClose);
        true
    }
}

fn handle_close(stack: &mut Vec<ContainerState>, is_brace: bool) -> bool {
    let Some(top) = stack.pop() else {
        return false;
    };
    if is_brace {
        top == ContainerState::KeyOrClose || top == ContainerState::CommaOrClose
    } else {
        top == ContainerState::ArrValueOrClose || top == ContainerState::ArrCommaOrClose
    }
}

fn handle_colon_or_comma(stack: &mut [ContainerState], c: char) -> bool {
    let Some(top) = stack.last_mut() else {
        return false;
    };
    if c == ':' {
        if *top == ContainerState::Colon {
            *top = ContainerState::Value;
            true
        } else {
            false
        }
    } else {
        match top {
            ContainerState::CommaOrClose => {
                *top = ContainerState::KeyOrClose;
                true
            }
            ContainerState::ArrCommaOrClose => {
                *top = ContainerState::ArrValueOrClose;
                true
            }
            _ => false,
        }
    }
}

fn handle_literal_char(stack: &mut [ContainerState]) -> bool {
    if let Some(top) = stack.last_mut() {
        if *top == ContainerState::Value {
            *top = ContainerState::CommaOrClose;
            true
        } else if *top == ContainerState::ArrValueOrClose {
            *top = ContainerState::ArrCommaOrClose;
            true
        } else {
            *top == ContainerState::CommaOrClose || *top == ContainerState::ArrCommaOrClose
        }
    } else {
        false
    }
}

/// Tokenizer-based prefix validation for JSON strings.
fn validate_json_prefix_tokens(text: &str) -> bool {
    let mut in_string = false;
    let mut is_escaped = false;
    let mut stack: Vec<ContainerState> = Vec::new();

    for c in text.chars() {
        if in_string {
            if is_escaped {
                is_escaped = false;
            } else if c == '\\' {
                is_escaped = true;
            } else if c == '"' {
                in_string = false;
                update_string_close_state(&mut stack);
            }
            continue;
        }

        let valid = match c {
            '"' => match stack.last() {
                Some(
                    ContainerState::KeyOrClose
                    | ContainerState::Value
                    | ContainerState::ArrValueOrClose,
                ) => {
                    in_string = true;
                    true
                }
                _ => false,
            },
            '{' => handle_open_brace(&mut stack),
            '}' => handle_close(&mut stack, true),
            '[' => handle_open_bracket(&mut stack),
            ']' => handle_close(&mut stack, false),
            ':' | ',' => handle_colon_or_comma(&mut stack, c),
            ' ' | '\t' | '\n' | '\r' => true,
            '-' | '0'..='9' | '.' | 'e' | 'E' | '+' => handle_literal_char(&mut stack),
            't' | 'r' | 'u' | 'f' | 'a' | 'l' | 's' | 'n' => handle_literal_char(&mut stack),
            _ => false,
        };

        if !valid {
            return false;
        }
    }

    true
}

fn update_string_close_state(stack: &mut [ContainerState]) {
    if let Some(top) = stack.last_mut() {
        match top {
            ContainerState::KeyOrClose => *top = ContainerState::Colon,
            ContainerState::Value => *top = ContainerState::CommaOrClose,
            ContainerState::ArrValueOrClose => *top = ContainerState::ArrCommaOrClose,
            _ => {}
        }
    }
}

/// Validates complete JSON Value against required fields and basic types in schema.
fn validate_complete_value(val: &Value, schema: &Value) -> bool {
    if let Some(expected_type) = schema.get("type").and_then(|t| t.as_str()) {
        match expected_type {
            "object" => {
                let Some(obj) = val.as_object() else {
                    return false;
                };
                if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
                    for req in required {
                        if let Some(key) = req.as_str()
                            && !obj.contains_key(key)
                        {
                            return false;
                        }
                    }
                }
            }
            "array" => return val.is_array(),
            "string" => return val.is_string(),
            "number" | "integer" => return val.is_number(),
            "boolean" => return val.is_boolean(),
            _ => {}
        }
    }
    true
}
