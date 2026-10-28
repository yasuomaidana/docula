//! Common constrained response generator and schema DFA validator.

use crate::error::LlmError;
use docula_schema::{DfaLogitsMasker, SchemaDfa};
use schemars::Schema;
use std::path::Path;

/// Verifies canned response conformity against the compiled schema DFA.
pub fn verify_canned_or_model_path(
    engine_name: &'static str,
    canned_response: Option<&str>,
    model_path: &Path,
    schema: &Schema,
) -> Result<Option<String>, LlmError> {
    let dfa = SchemaDfa::compile(schema)?;
    let masker = DfaLogitsMasker::new(dfa);

    if let Some(canned) = canned_response {
        if !masker.dfa().is_accepted(canned) {
            return Err(LlmError::InferenceFailure(format!(
                "{engine_name} response does not conform to the required JSON schema"
            )));
        }
        return Ok(Some(canned.to_string()));
    }

    if model_path.to_string_lossy() != ":memory:" && !model_path.exists() {
        return Err(LlmError::ModelNotFound(format!(
            "{engine_name} weights file not found at: {}",
            model_path.display()
        )));
    }

    Ok(None)
}
