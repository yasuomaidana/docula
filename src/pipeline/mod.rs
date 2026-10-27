//! # docula::pipeline
//!
//! High-level end-to-end document intelligence pipeline orchestrating ingestion,
//! rasterization, optical code scanning, OCR, compliance redaction, schema DFA masking,
//! few-shot exemplar injection, self-repair loops, multi-shot document reconstruction,
//! and single-shot multi-document instance segmentation.

pub mod builder;
pub mod error;
pub mod stages;

pub use builder::DoculaPipelineBuilder;
pub use error::DoculaPipelineError;

use docula_compliance::ComplianceRedactor;
use docula_core::{DocumentSource, SpatialSpan};
use docula_exemplar::ExemplarPool;
use docula_hitl::{TriageDecision, TriageGate, TriageItem, TriageQueue};
use docula_ingest::{DocumentRouter, PageRoute};
use docula_llm::{LlmBackend, MockLlmBackend};
use docula_ocr::{MockOcrEngine, OcrEngine, SpatialLayoutMap};
use docula_preprocess::{OfficeRasterizer, PageRasterBuffer};
use docula_rag::{DocumentChunk, RagRetriever};
use docula_schema::{
    DocuValidate, ValidationError, ValidationMode, ValidationOutcome,
};
use docula_vision_codes::{OpticalCodeDetector, OpticalCodeResult};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::sync::Arc;
use uuid::Uuid;

/// Operational configuration parameters for [`DoculaPipeline`].
#[derive(Clone, Debug)]
pub struct PipelineConfig {
    pub dpi: u32,
    pub max_repair_attempts: usize,
    pub min_token_confidence: f32,
    pub enable_redaction: bool,
    pub enable_vision_codes: bool,
    pub enable_exemplars: bool,
    pub enable_rag: bool,
    pub validation_mode: ValidationMode,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            dpi: 150,
            max_repair_attempts: 3,
            min_token_confidence: 0.85,
            enable_redaction: true,
            enable_vision_codes: true,
            enable_exemplars: true,
            enable_rag: false,
            validation_mode: ValidationMode::Strict,
        }
    }
}

/// End-to-end extraction result payload emitted by [`DoculaPipeline`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtractionResult<T> {
    pub document_id: String,
    pub outcome: ValidationOutcome<T>,
    pub data: Option<T>,
    pub raw_json: String,
    pub confidence: f32,
    pub optical_codes: Vec<OpticalCodeResult>,
    pub formatted_text: String,
    pub repair_attempts: usize,
    pub routed_to_triage: bool,
    pub triage_reasons: Vec<String>,
    pub rag_applied: bool,
    pub rag_retrieved_chunks: Vec<DocumentChunk>,
}

/// High-level sovereign document intelligence pipeline facade.
pub struct DoculaPipeline {
    pub(crate) config: PipelineConfig,
    pub(crate) router: DocumentRouter,
    pub(crate) office_rasterizer: OfficeRasterizer,
    pub(crate) ocr_engine: Arc<dyn OcrEngine>,
    pub(crate) code_detector: OpticalCodeDetector,
    pub(crate) compliance_redactor: ComplianceRedactor,
    pub(crate) _exemplar_pool: ExemplarPool,
    pub(crate) triage_gate: TriageGate,
    pub(crate) triage_queue: TriageQueue,
    pub(crate) llm_backend: Arc<dyn LlmBackend>,
    pub(crate) rag_retriever: Option<RagRetriever>,
}

impl Default for DoculaPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl DoculaPipeline {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: PipelineConfig::default(),
            router: DocumentRouter::new(),
            office_rasterizer: OfficeRasterizer::new(),
            ocr_engine: Arc::new(MockOcrEngine::new()),
            code_detector: OpticalCodeDetector::new(),
            compliance_redactor: ComplianceRedactor::default(),
            _exemplar_pool: ExemplarPool::new(),
            triage_gate: TriageGate::new(0.85),
            triage_queue: TriageQueue::new(),
            llm_backend: Arc::new(MockLlmBackend::new()),
            rag_retriever: None,
        }
    }

    #[must_use]
    pub fn builder() -> DoculaPipelineBuilder {
        DoculaPipelineBuilder::default()
    }

    #[inline]
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }

    #[inline]
    pub fn triage_queue(&self) -> &TriageQueue {
        &self.triage_queue
    }

    pub async fn process<T>(
        &self,
        source: &DocumentSource,
    ) -> Result<ExtractionResult<T>, DoculaPipelineError>
    where
        T: DocuValidate + schemars::JsonSchema + DeserializeOwned + Serialize + Send + Sync + Clone,
    {
        let inferred_id = source.infer_document_id();
        self.process_document(&inferred_id, source).await
    }

    pub async fn process_document<T>(
        &self,
        document_id: &str,
        source: &DocumentSource,
    ) -> Result<ExtractionResult<T>, DoculaPipelineError>
    where
        T: DocuValidate + schemars::JsonSchema + DeserializeOwned + Serialize + Send + Sync + Clone,
    {
        let routing_report = self.router.route_source(source)?;
        let first_route = routing_report.routes.first();

        let (buffer, spans) = match first_route {
            Some(PageRoute::BornDigital { text_layer, .. }) if !source.is_pdf() => {
                let spans = text_layer.clone();
                let buf = self
                    .office_rasterizer
                    .render_spans(&spans, 0, self.config.dpi)
                    .map_err(DoculaPipelineError::Preprocess)?;
                (buf, spans)
            }
            Some(PageRoute::BornDigital { text_layer, .. }) => {
                let buf = self.render_or_load_source(source)?;
                let spans = text_layer.clone();
                (buf, spans)
            }
            _ => {
                let buf = self.render_or_load_source(source)?;
                let spans = self.ocr_engine.extract_spans(&buf.image)?;
                (buf, spans)
            }
        };

        self.extract_with_prepared_spans(document_id, buffer, spans)
            .await
    }

    pub async fn extract_from_buffer<T>(
        &self,
        document_id: &str,
        buffer: &PageRasterBuffer,
    ) -> Result<ExtractionResult<T>, DoculaPipelineError>
    where
        T: DocuValidate + schemars::JsonSchema + DeserializeOwned + Serialize + Send + Sync + Clone,
    {
        let spans = self.ocr_engine.extract_spans(&buffer.image)?;
        self.extract_with_prepared_spans(document_id, buffer.clone(), spans)
            .await
    }

    pub fn render_or_load_source(
        &self,
        source: &DocumentSource,
    ) -> Result<PageRasterBuffer, DoculaPipelineError> {
        if source.is_office() {
            let routing_report = self.router.route_source(source)?;
            if let Some(PageRoute::BornDigital { text_layer, .. }) = routing_report.routes.first() {
                return self
                    .office_rasterizer
                    .render_spans(text_layer, 0, self.config.dpi)
                    .map_err(DoculaPipelineError::Preprocess);
            }
        }

        PageRasterBuffer::from_document_source(source, self.config.dpi)
            .map_err(DoculaPipelineError::Preprocess)
    }

    async fn extract_with_prepared_spans<T>(
        &self,
        document_id: &str,
        mut buffer: PageRasterBuffer,
        mut spans: Vec<SpatialSpan>,
    ) -> Result<ExtractionResult<T>, DoculaPipelineError>
    where
        T: DocuValidate + schemars::JsonSchema + DeserializeOwned + Serialize + Send + Sync + Clone,
    {
        let optical_codes = if self.config.enable_vision_codes {
            self.code_detector.detect(&buffer.image)?
        } else {
            Vec::new()
        };

        if self.config.enable_redaction {
            self.compliance_redactor
                .auto_censor(&mut buffer.image, &mut spans);
        }

        let layout_map = SpatialLayoutMap::from_spans(spans, buffer.dimensions());
        let formatted_text = layout_map.to_coordinate_formatted_text();

        let mut rag_applied = false;
        let mut rag_retrieved_chunks = Vec::new();
        let mut rag_context = String::new();

        if self.config.enable_rag {
            let mut retriever = self.rag_retriever.clone().unwrap_or_default();
            retriever.index_page(0, &layout_map);

            let query = format!("{document_id} {formatted_text}");
            let mut chunks = retriever.retrieve(&query, None);
            if chunks.is_empty()
                && !layout_map.is_empty()
                && let Some(first_line) = layout_map.lines.first()
            {
                chunks = retriever.retrieve(&first_line.text, None);
            }

            if !chunks.is_empty() {
                rag_applied = true;
                rag_retrieved_chunks = chunks;
                for chunk in &rag_retrieved_chunks {
                    rag_context.push_str(&chunk.text);
                    rag_context.push('\n');
                }
            }
        }

        let prompt_context = build_prompt_context(&formatted_text, &optical_codes, &rag_context);
        let schema = schemars::schema_for!(T);
        let (outcome, final_data, final_json, repair_attempts, domain_errors) = self
            .run_self_repair_loop::<T>(&prompt_context, &schema)
            .await?;

        let confidence = if outcome.is_strict() {
            0.95
        } else if outcome.is_soft() {
            0.75
        } else {
            0.20
        };

        let (routed_to_triage, triage_reasons) =
            if self.config.validation_mode == ValidationMode::Permissive {
                (false, Vec::new())
            } else {
                self.evaluate_triage(&layout_map, &final_json, confidence, &domain_errors)
            };

        Ok(ExtractionResult {
            document_id: document_id.to_string(),
            outcome,
            data: final_data,
            raw_json: final_json,
            confidence,
            optical_codes,
            formatted_text,
            repair_attempts,
            routed_to_triage,
            triage_reasons,
            rag_applied,
            rag_retrieved_chunks,
        })
    }

    async fn run_self_repair_loop<T>(
        &self,
        prompt_context: &str,
        schema: &schemars::Schema,
    ) -> Result<
        (
            ValidationOutcome<T>,
            Option<T>,
            String,
            usize,
            Vec<ValidationError>,
        ),
        DoculaPipelineError,
    >
    where
        T: DocuValidate + DeserializeOwned + Serialize + Send + Sync + Clone,
    {
        let system_prompt = "You are a precise document extraction engine. Return strictly valid JSON matching the schema.";
        let mut user_prompt = prompt_context.to_string();
        let mut final_json = String::new();
        let mut last_parsed: Option<T> = None;
        let mut domain_errors = Vec::new();
        let mut attempts = 0;
        let mut strict_passed = false;

        for step in 0..self.config.max_repair_attempts {
            attempts = step + 1;
            let generated = self
                .llm_backend
                .generate_constrained(system_prompt, &user_prompt, schema)
                .await?;
            final_json = generated.clone();

            match serde_json::from_str::<T>(&generated) {
                Ok(parsed) => match parsed.validate_integrity() {
                    Ok(()) => {
                        last_parsed = Some(parsed);
                        domain_errors.clear();
                        strict_passed = true;
                        break;
                    }
                    Err(errs) => {
                        domain_errors = errs;
                        last_parsed = Some(parsed);
                        append_repair_feedback(&mut user_prompt, &domain_errors);
                    }
                },
                Err(syntax_err) => {
                    domain_errors =
                        vec![ValidationError::new("json_syntax", syntax_err.to_string())];
                    user_prompt.push_str(&format!(
                        "\n\nJSON SYNTAX ERROR: {syntax_err}\nPlease correct formatting."
                    ));
                }
            }
        }

        let outcome = if strict_passed && let Some(valid_data) = last_parsed.clone() {
            ValidationOutcome::Strict(valid_data)
        } else if let Some(soft_data) = last_parsed.clone() {
            ValidationOutcome::Soft {
                data: soft_data,
                validation_errors: domain_errors.clone(),
            }
        } else {
            ValidationOutcome::Error {
                raw_output: final_json.clone(),
                message: domain_errors
                    .first()
                    .map(|e| e.error_message.clone())
                    .unwrap_or_else(|| "Syntax or schema failure".to_string()),
            }
        };

        let final_data = match self.config.validation_mode {
            ValidationMode::Strict => {
                if strict_passed {
                    last_parsed
                } else {
                    None
                }
            }
            ValidationMode::Soft | ValidationMode::Permissive => last_parsed,
        };

        Ok((outcome, final_data, final_json, attempts, domain_errors))
    }

    fn evaluate_triage(
        &self,
        layout_map: &SpatialLayoutMap,
        final_json: &str,
        confidence: f32,
        domain_errors: &[ValidationError],
    ) -> (bool, Vec<String>) {
        let decision = self.triage_gate.evaluate(confidence, domain_errors);
        match decision {
            TriageDecision::Pass => (false, Vec::new()),
            TriageDecision::NeedsReview { reasons } => {
                let triage_item = TriageItem {
                    id: Uuid::now_v7(),
                    model_output: serde_json::from_str(final_json)
                        .unwrap_or(serde_json::Value::Null),
                    reasons: reasons.clone(),
                    spatial_layout: layout_map.clone(),
                    confidence,
                };
                self.triage_queue.enqueue(triage_item);
                (true, reasons)
            }
        }
    }
}

fn append_repair_feedback(user_prompt: &mut String, domain_errors: &[ValidationError]) {
    user_prompt.push_str("\n\nINTEGRITY VALIDATION REPAIR REQUIRED:\n");
    for err in domain_errors {
        user_prompt.push_str(&format!("- [{}] {}\n", err.field_path, err.error_message));
    }
    user_prompt.push_str("Please correct these discrepancies and output valid JSON.");
}

fn build_prompt_context(
    formatted_text: &str,
    codes: &[OpticalCodeResult],
    rag_context: &str,
) -> String {
    let mut context = format!("DOCUMENT SPATIAL LAYOUT:\n{formatted_text}\n");
    if !rag_context.is_empty() {
        context.push_str("\nRETRIEVAL-AUGMENTED CONTEXT (RAG):\n");
        context.push_str(rag_context);
        context.push('\n');
    }
    if !codes.is_empty() {
        context.push_str("\nDECODED OPTICAL CODES & FINANCIAL PAYLOADS:\n");
        for code in codes {
            context.push_str(&format!(
                "- Format: {:?}, Raw: {}\n",
                code.code_type, code.raw_content
            ));
            if let Some(ref fin) = code.financial_payload {
                context.push_str(&format!("  Financial: {fin:?}\n"));
            }
        }
    }
    context
}
