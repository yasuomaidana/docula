//! Pipeline builder.

use super::{DoculaPipeline, PipelineConfig};
use docula_compliance::ComplianceRedactor;
use docula_exemplar::ExemplarPool;
use docula_hitl::{TriageGate, TriageQueue};
use docula_ingest::DocumentRouter;
use docula_llm::{LlmBackend, MockLlmBackend};
use docula_ocr::{MockOcrEngine, OcrEngine};
use docula_preprocess::OfficeRasterizer;
use docula_rag::RagRetriever;
use docula_vision_codes::OpticalCodeDetector;
use std::sync::Arc;

/// Fluent builder for constructing customized [`DoculaPipeline`] instances.
#[derive(Default)]
pub struct DoculaPipelineBuilder {
    config: PipelineConfig,
    router: Option<DocumentRouter>,
    ocr_engine: Option<Arc<dyn OcrEngine>>,
    llm_backend: Option<Arc<dyn LlmBackend>>,
    compliance_redactor: Option<ComplianceRedactor>,
    exemplar_pool: Option<ExemplarPool>,
    rag_retriever: Option<RagRetriever>,
}

impl DoculaPipelineBuilder {
    #[must_use]
    pub fn with_config(mut self, config: PipelineConfig) -> Self {
        self.config = config;
        self
    }

    #[must_use]
    pub fn with_router(mut self, router: DocumentRouter) -> Self {
        self.router = Some(router);
        self
    }

    #[must_use]
    pub fn with_ocr_engine(mut self, ocr_engine: Arc<dyn OcrEngine>) -> Self {
        self.ocr_engine = Some(ocr_engine);
        self
    }

    #[must_use]
    pub fn with_llm_backend(mut self, llm_backend: Arc<dyn LlmBackend>) -> Self {
        self.llm_backend = Some(llm_backend);
        self
    }

    #[must_use]
    pub fn with_compliance_redactor(mut self, redactor: ComplianceRedactor) -> Self {
        self.compliance_redactor = Some(redactor);
        self
    }

    #[must_use]
    pub fn with_exemplar_pool(mut self, pool: ExemplarPool) -> Self {
        self.exemplar_pool = Some(pool);
        self
    }

    #[must_use]
    pub fn with_rag_retriever(mut self, retriever: RagRetriever) -> Self {
        self.rag_retriever = Some(retriever);
        self
    }

    pub fn build(self) -> DoculaPipeline {
        DoculaPipeline {
            triage_gate: TriageGate::new(self.config.min_token_confidence),
            config: self.config,
            router: self.router.unwrap_or_default(),
            office_rasterizer: OfficeRasterizer::new(),
            ocr_engine: self
                .ocr_engine
                .unwrap_or_else(|| Arc::new(MockOcrEngine::new())),
            code_detector: OpticalCodeDetector::new(),
            compliance_redactor: self.compliance_redactor.unwrap_or_default(),
            _exemplar_pool: self.exemplar_pool.unwrap_or_default(),
            triage_queue: TriageQueue::new(),
            llm_backend: self
                .llm_backend
                .unwrap_or_else(|| Arc::new(MockLlmBackend::new())),
            rag_retriever: self.rag_retriever,
        }
    }
}
