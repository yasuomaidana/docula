//! # docula
//!
//! Sovereign, pure-Rust zero-FFI document intelligence engine.

pub mod pipeline;

pub use docula_compliance as compliance;
pub use docula_core::*;
pub use docula_domains as domains;
pub use docula_exemplar as exemplar;
pub use docula_hitl as hitl;
pub use docula_ingest as ingest;
pub use docula_llm as llm;
pub use docula_ocr as ocr;
pub use docula_preprocess as preprocess;
pub use docula_rag as rag;
pub use docula_schema as schema;
pub use docula_vision_codes as vision_codes;
pub use pipeline::{DoculaPipeline, DoculaPipelineBuilder, ExtractionResult, PipelineConfig};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_reexports() {
        let point = Point::new(0.1, 0.2).expect("valid normalized point");
        assert_eq!(point.x, 0.1);
        assert_eq!(point.y, 0.2);

        let rasterizer = preprocess::TinySkiaRasterizer::new();
        let _ = rasterizer;

        let router = ingest::DocumentRouter::new();
        let _ = router;

        let mock_ocr = ocr::MockOcrEngine::new();
        let _ = mock_ocr;

        let redactor = compliance::ComplianceRedactor::default();
        let _ = redactor;

        let dfa = schema::SchemaDfa::compile_json(r#"{"type":"object"}"#).unwrap();
        assert_eq!(dfa.root_type(), "object");

        let mock_llm = llm::MockLlmBackend::new();
        let _ = mock_llm;

        let retriever = rag::RagRetriever::default();
        let _ = retriever;

        let exemplar_pool = exemplar::ExemplarPool::new();
        let _ = exemplar_pool;

        let triage_gate = hitl::TriageGate::default();
        let _ = triage_gate;

        let code_detector = vision_codes::OpticalCodeDetector::new();
        let _ = code_detector;

        let invoice_item = domains::LineItem::new("Widget", 2.0, 10.0);
        assert_eq!(invoice_item.total, 20.0);

        let pipeline = DoculaPipeline::new();
        assert_eq!(pipeline.config().dpi, 150);
    }
}
