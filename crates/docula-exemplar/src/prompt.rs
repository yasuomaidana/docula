use crate::template::TemplatePair;
use docula_ocr::SpatialLayoutMap;

/// Builder constructing LLM context prompts with dynamically injected few-shot exemplars.
pub struct FewShotPromptBuilder;

impl FewShotPromptBuilder {
    /// Builds a prompt from system instructions, optional retrieved exemplars, and target layout.
    ///
    /// When `exemplars` is empty, output collapses cleanly into zero-shot mode without any overhead tokens.
    #[must_use]
    pub fn build(
        system_instructions: &str,
        exemplars: &[TemplatePair],
        target_spatial_map: &SpatialLayoutMap,
    ) -> String {
        let mut prompt = String::with_capacity(4096);
        prompt.push_str(system_instructions);
        prompt.push_str("\n\n");

        if !exemplars.is_empty() {
            prompt.push_str("### REFERENCE DOCUMENT EXAMPLES:\n");
            prompt.push_str(
                "Learn the spatial key-value mappings and layout structures from these verified examples.\n\n",
            );

            for (idx, ex) in exemplars.iter().enumerate() {
                prompt.push_str(&format!(
                    "--- [Example {}] Document Type: {} ---\n",
                    idx + 1,
                    ex.metadata.document_type
                ));
                prompt.push_str("Spatial Layout Input:\n");
                prompt.push_str(&ex.spatial_layout.to_coordinate_formatted_text());
                prompt.push_str("\nTarget JSON Output:\n");
                prompt.push_str(
                    &serde_json::to_string_pretty(&ex.ground_truth_json).unwrap_or_default(),
                );
                prompt.push_str("\n\n");
            }

            prompt.push_str("### TARGET DOCUMENT FOR EXTRACTION:\n");
        }

        prompt.push_str(
            "Extract all data strictly matching the requested schema from this document:\n",
        );
        prompt.push_str(&target_spatial_map.to_coordinate_formatted_text());
        prompt.push_str("\nJSON Output:");
        prompt
    }
}
