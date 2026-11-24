use crate::chunk::{DocumentChunk, TableAwareChunker};
use crate::index::HybridIndex;
use docula_ocr::SpatialLayoutMap;
use serde::{Deserialize, Serialize};

/// Configuration options governing layout chunking and hybrid retrieval.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RagConfig {
    /// Number of top relevant chunks to retrieve.
    pub top_k_chunks: usize,
    /// Minimum threshold score for candidate chunks.
    pub min_similarity: f32,
    /// Weight given to dense vectors vs BM25 keywords in [0.0, 1.0].
    pub hybrid_weight: f32,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            top_k_chunks: 5,
            min_similarity: 0.1,
            hybrid_weight: 0.5,
        }
    }
}

/// Orchestrator coordinating table-aware document chunking and hybrid retrieval.
#[derive(Clone, Debug, Default)]
pub struct RagRetriever {
    chunker: TableAwareChunker,
    index: HybridIndex,
    config: RagConfig,
}

impl RagRetriever {
    /// Creates a new `RagRetriever` with the specified configuration.
    #[must_use]
    pub fn new(config: RagConfig) -> Self {
        Self {
            chunker: TableAwareChunker::default(),
            index: HybridIndex::new(),
            config,
        }
    }

    /// Indexes a page's `SpatialLayoutMap` into the internal hybrid index.
    pub fn index_page(&mut self, page_index: usize, layout: &SpatialLayoutMap) {
        let start_id = self.index.len();
        let chunks = self.chunker.chunk_layout(page_index, layout, start_id);
        for chunk in chunks {
            self.index.add_chunk(chunk, None);
        }
    }

    /// Indexes a chunk with a precomputed dense vector embedding.
    pub fn add_chunk_with_embedding(&mut self, chunk: DocumentChunk, embedding: Vec<f32>) {
        self.index.add_chunk(chunk, Some(embedding));
    }

    /// Retrieves the top-$k$ relevant chunks matching the query text.
    #[must_use]
    pub fn retrieve(
        &self,
        query_text: &str,
        query_embedding: Option<&[f32]>,
    ) -> Vec<DocumentChunk> {
        let scored = self.index.query(
            query_text,
            query_embedding,
            self.config.top_k_chunks,
            self.config.hybrid_weight,
        );

        scored
            .into_iter()
            .filter(|(_, score)| *score >= self.config.min_similarity)
            .map(|(chunk, _)| chunk)
            .collect()
    }

    /// Assembles a focused context prompt window from retrieved chunks.
    #[must_use]
    pub fn build_context(&self, query_text: &str) -> String {
        let chunks = self.retrieve(query_text, None);
        let mut context = String::new();
        for chunk in chunks {
            context.push_str(&chunk.text);
            context.push('\n');
        }
        context
    }
}
