//! Adapter bridging `docula` document chunks into `embedag-rag` hybrid retrieval and graph expansion.

#[cfg(feature = "embedag-integration")]
use crate::chunk::DocumentChunk;
#[cfg(feature = "embedag-integration")]
use crate::error::RagError;
#[cfg(feature = "embedag-integration")]
use embedag_rag::{DenseVectorIndex, RrfRanker, SparseLexicalIndex, VerifiedRagEngine};
#[cfg(feature = "embedag-integration")]
use uuid::Uuid;

/// Adapter leveraging `embedag-rag` for reciprocal rank fusion (RRF) and verified retrieval.
#[cfg(feature = "embedag-integration")]
pub struct EmbedagGraphRetriever {
    chunks: Vec<(Uuid, DocumentChunk)>,
    dense_index: DenseVectorIndex,
    lexical_index: SparseLexicalIndex,
    verified_engine: VerifiedRagEngine,
}

#[cfg(feature = "embedag-integration")]
impl Default for EmbedagGraphRetriever {
    fn default() -> Self {
        Self::new(384)
    }
}

#[cfg(feature = "embedag-integration")]
impl EmbedagGraphRetriever {
    /// Creates a new `EmbedagGraphRetriever` with specified vector embedding dimensions.
    #[must_use]
    pub fn new(embedding_dim: usize) -> Self {
        Self {
            chunks: Vec::new(),
            dense_index: DenseVectorIndex::with_dimensions(embedding_dim),
            lexical_index: SparseLexicalIndex::new(),
            verified_engine: VerifiedRagEngine::new(embedding_dim),
        }
    }

    /// Adds a layout-aware document chunk to the retriever, indexing its text lexically and densely.
    pub fn add_chunk(&mut self, chunk: DocumentChunk, embedding: Option<Vec<f32>>) {
        let chunk_uuid = Uuid::from_u128((chunk.id as u128) + 1);

        self.lexical_index
            .insert(chunk_uuid, format!("chunk_{}", chunk.id), &chunk.text);

        if let Some(emb) = embedding {
            let _ = self.dense_index.insert(chunk_uuid, emb);
        }

        self.chunks.push((chunk_uuid, chunk));
    }

    /// Retrieves top-$k$ relevant chunks using `embedag-rag`'s Reciprocal Rank Fusion (RRF).
    #[must_use]
    pub fn retrieve_rrf(
        &self,
        query_text: &str,
        query_embedding: Option<&[f32]>,
        top_k: usize,
    ) -> Vec<(DocumentChunk, f32)> {
        let lexical_results = self.lexical_index.search(query_text, top_k * 2);
        let lexical_ids: Vec<Uuid> = lexical_results.into_iter().map(|(id, _)| id).collect();

        let dense_ids: Vec<Uuid> = if let Some(query_vec) = query_embedding {
            self.dense_index
                .search(query_vec, top_k * 2)
                .unwrap_or_default()
                .into_iter()
                .map(|(id, _)| id)
                .collect()
        } else {
            Vec::new()
        };

        let ranker = RrfRanker::default();
        let mut candidates = ranker.fuse(&dense_ids, &lexical_ids);
        candidates.truncate(top_k);

        candidates
            .into_iter()
            .filter_map(|cand| {
                self.chunks
                    .iter()
                    .find(|(id, _)| *id == cand.id)
                    .map(|(_, chunk)| (chunk.clone(), cand.score))
            })
            .collect()
    }

    /// Executes verified retrieval over the indexed document chunks using `embedag-rag`'s verified retrieval engine.
    pub async fn query_verified(&self, query_text: &str) -> Result<String, RagError> {
        let corpus: Vec<&str> = self.chunks.iter().map(|(_, c)| c.text.as_str()).collect();
        self.verified_engine
            .query_verified(&corpus, query_text)
            .await
            .map_err(Into::into)
    }

    /// Assembles a focused context prompt window from retrieved RRF chunks.
    #[must_use]
    pub fn build_context(&self, query_text: &str, top_k: usize) -> String {
        let results = self.retrieve_rrf(query_text, None, top_k);
        let mut context = String::new();
        for (chunk, _) in results {
            context.push_str(&chunk.text);
            context.push('\n');
        }
        context
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use docula_core::BoundingBox;

    #[test]
    fn test_embedag_adapter_rrf_retrieval() {
        let mut retriever = EmbedagGraphRetriever::new(384);
        let chunk1 = DocumentChunk {
            id: 0,
            text: "Invoice #1042 Total amount due: $500.00".to_string(),
            page_index: 0,
            bbox: BoundingBox::from_coords_clamped(0.0, 0.0, 1.0, 0.2),
            is_table: false,
            token_estimate: 10,
        };
        let chunk2 = DocumentChunk {
            id: 1,
            text: "Terms and conditions payment within 30 days".to_string(),
            page_index: 0,
            bbox: BoundingBox::from_coords_clamped(0.0, 0.2, 1.0, 0.4),
            is_table: false,
            token_estimate: 10,
        };

        retriever.add_chunk(chunk1.clone(), None);
        retriever.add_chunk(chunk2.clone(), None);

        let retrieved = retriever.retrieve_rrf("Invoice amount", None, 1);
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].0.id, 0);
        assert!(retrieved[0].1 > 0.0);
    }
}
