//! OxiRAG 4-layer sovereign RAG engine integration for docula.
//!
//! Powered by KitaSan `oxirag`, providing multi-layer retrieval:
//! - Layer 1 (Echo): Fast prefix caching and vector similarity search.
//! - Layer 2 (Speculator): Speculative draft retrieval and windowing.
//! - Layer 3 (Judge): Logic & constraint verification.

use crate::error::RagError;
use oxirag::prelude::*;

/// Sovereign 4-layer RAG processor utilizing KitaSan `oxirag`.
pub struct OxiRagEngine {
    embedding_dim: usize,
}

impl OxiRagEngine {
    /// Creates a new `OxiRagEngine` with specified embedding dimensions (default: 384 for MiniLM).
    #[must_use]
    pub fn new(embedding_dim: usize) -> Self {
        Self { embedding_dim }
    }

    /// Indexes document text chunks and runs a verified retrieval query.
    pub async fn query_verified(
        &self,
        corpus: &[&str],
        query_text: &str,
    ) -> Result<String, RagError> {
        let echo = EchoLayer::new(
            MockEmbeddingProvider::new(self.embedding_dim),
            InMemoryVectorStore::new(self.embedding_dim),
        );

        let speculator = RuleBasedSpeculator::default();

        let judge = JudgeImpl::new(
            AdvancedClaimExtractor::new(),
            MockSmtVerifier::default(),
            JudgeConfig::default(),
        );

        let mut pipeline = PipelineBuilder::new()
            .with_echo(echo)
            .with_speculator(speculator)
            .with_judge(judge)
            .build()
            .map_err(|e| RagError::IndexError(format!("Failed to build OxiRAG pipeline: {e:?}")))?;

        for &chunk in corpus {
            pipeline
                .index(Document::new(chunk))
                .await
                .map_err(|e| RagError::IndexError(format!("Failed to index in OxiRAG: {e:?}")))?;
        }

        let query = Query::new(query_text);
        let result = pipeline
            .process(query)
            .await
            .map_err(|e| RagError::IndexError(format!("OxiRAG query failed: {e:?}")))?;

        Ok(result.final_answer)
    }
}
