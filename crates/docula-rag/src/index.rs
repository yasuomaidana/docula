use crate::chunk::DocumentChunk;
use std::collections::HashMap;

/// In-memory hybrid index combining BM25 keyword scoring and dense vector cosine similarity.
#[derive(Clone, Debug, Default)]
pub struct HybridIndex {
    chunks: Vec<DocumentChunk>,
    embeddings: Vec<Option<Vec<f32>>>,
    // BM25 index components
    doc_lengths: Vec<usize>,
    avg_doc_length: f32,
    term_doc_frequencies: HashMap<String, usize>,
    term_chunk_frequencies: Vec<HashMap<String, usize>>,
}

impl HybridIndex {
    /// Constructs a new empty `HybridIndex`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of indexed chunks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Returns `true` if index contains zero chunks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Inserts a chunk into the index with an optional dense vector embedding.
    pub fn add_chunk(&mut self, chunk: DocumentChunk, embedding: Option<Vec<f32>>) {
        let terms = tokenize(&chunk.text);
        let doc_len = terms.len();
        let mut term_counts: HashMap<String, usize> = HashMap::new();

        for term in terms {
            *term_counts.entry(term).or_insert(0) += 1;
        }

        for term in term_counts.keys() {
            *self.term_doc_frequencies.entry(term.clone()).or_insert(0) += 1;
        }

        self.doc_lengths.push(doc_len);
        let total_len: usize = self.doc_lengths.iter().sum();
        self.avg_doc_length = if self.doc_lengths.is_empty() {
            0.0
        } else {
            total_len as f32 / self.doc_lengths.len() as f32
        };

        self.term_chunk_frequencies.push(term_counts);
        self.chunks.push(chunk);
        self.embeddings.push(embedding);
    }

    /// Queries the hybrid index using text keywords and optional dense vector query.
    ///
    /// `hybrid_weight`: balance between BM25 (0.0) and dense vector (1.0).
    #[must_use]
    pub fn query(
        &self,
        query_text: &str,
        query_embedding: Option<&[f32]>,
        top_k: usize,
        hybrid_weight: f32,
    ) -> Vec<(DocumentChunk, f32)> {
        if self.chunks.is_empty() {
            return Vec::new();
        }

        let query_terms = tokenize(query_text);
        let bm25_scores = self.compute_bm25_scores(&query_terms);
        let vec_scores = self.compute_vector_scores(query_embedding);

        let mut scored_chunks = Vec::with_capacity(self.chunks.len());
        for i in 0..self.chunks.len() {
            let bm25_norm = bm25_scores[i];
            let vec_sim = vec_scores[i];

            let combined_score = if query_embedding.is_some() {
                (1.0 - hybrid_weight) * bm25_norm + hybrid_weight * vec_sim
            } else {
                bm25_norm
            };

            scored_chunks.push((self.chunks[i].clone(), combined_score));
        }

        scored_chunks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_chunks.truncate(top_k);
        scored_chunks
    }

    fn compute_bm25_scores(&self, query_terms: &[String]) -> Vec<f32> {
        let n = self.chunks.len() as f32;
        let k1 = 1.2_f32;
        let b = 0.75_f32;

        let mut raw_scores = vec![0.0_f32; self.chunks.len()];
        let mut max_score = 0.0001_f32;

        for term in query_terms {
            if let Some(&df) = self.term_doc_frequencies.get(term) {
                let idf = ((n - df as f32 + 0.5) / (df as f32 + 0.5) + 1.0).ln();

                for (i, raw_score) in raw_scores.iter_mut().enumerate() {
                    let tf = *self.term_chunk_frequencies[i].get(term).unwrap_or(&0) as f32;
                    if tf > 0.0 {
                        let doc_len = self.doc_lengths[i] as f32;
                        let denom = tf
                            + k1 * (1.0 - b
                                + b * (if self.avg_doc_length > 0.0 {
                                    doc_len / self.avg_doc_length
                                } else {
                                    1.0
                                }));
                        let score = idf * (tf * (k1 + 1.0)) / denom;
                        *raw_score += score;
                        max_score = max_score.max(*raw_score);
                    }
                }
            }
        }

        // Normalize BM25 scores to [0.0, 1.0]
        raw_scores.iter().map(|&s| s / max_score).collect()
    }

    fn compute_vector_scores(&self, query_embedding: Option<&[f32]>) -> Vec<f32> {
        let Some(q_vec) = query_embedding else {
            return vec![0.0; self.chunks.len()];
        };

        self.embeddings
            .iter()
            .map(|emb_opt| {
                emb_opt
                    .as_deref()
                    .map(|doc_vec| cosine_similarity(q_vec, doc_vec).max(0.0))
                    .unwrap_or(0.0)
            })
            .collect()
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    for (&val_a, &val_b) in a.iter().zip(b.iter()) {
        dot += val_a * val_b;
        norm_a += val_a * val_a;
        norm_b += val_b * val_b;
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom > 0.0 { dot / denom } else { 0.0 }
}
