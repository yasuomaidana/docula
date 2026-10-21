use crate::fingerprint::{compute_layout_fingerprint, hamming_distance};
use crate::template::TemplatePair;
use docula_ocr::SpatialLayoutMap;

/// In-memory dual-stage exemplar retrieval pool.
#[derive(Clone, Debug, Default)]
pub struct ExemplarPool {
    templates: Vec<TemplatePair>,
    embeddings: Vec<Option<Vec<f32>>>,
    max_hamming_distance: u32,
    similarity_threshold: f32,
    max_injected: usize,
}

impl ExemplarPool {
    /// Creates a new `ExemplarPool` with default thresholds.
    #[must_use]
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
            embeddings: Vec::new(),
            max_hamming_distance: 3,
            similarity_threshold: 0.82,
            max_injected: 2,
        }
    }

    /// Registers a template pair into the pool with an optional precomputed embedding.
    pub fn register(&mut self, template: TemplatePair, embedding: Option<Vec<f32>>) {
        self.templates.push(template);
        self.embeddings.push(embedding);
    }

    /// Number of registered templates in the pool.
    #[must_use]
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    /// Returns `true` if pool contains no registered exemplars.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// Executes dual-stage retrieval for the target document.
    ///
    /// 1. Stage 1: Deterministic Vendor Tax ID or Layout Fingerprint match (sub-millisecond).
    /// 2. Stage 2: Vector cosine similarity fallback (if Stage 1 misses).
    /// 3. Returns empty if neither stage meets confidence threshold (zero-shot fallback).
    #[must_use]
    pub fn retrieve(
        &self,
        target_layout: &SpatialLayoutMap,
        vendor_tax_id: Option<&str>,
        target_embedding: Option<&[f32]>,
    ) -> Vec<TemplatePair> {
        if self.templates.is_empty() {
            return Vec::new();
        }

        // --- STAGE 1: Fast Deterministic Match (<1ms) ---
        // 1a. Vendor Tax ID exact match
        if let Some(tax_id) = vendor_tax_id {
            for template in &self.templates {
                if let Some(ref ex_tax) = template.metadata.vendor_tax_id
                    && ex_tax.eq_ignore_ascii_case(tax_id)
                {
                    return vec![template.clone()];
                }
            }
        }

        // 1b. Geometric SimHash layout fingerprint match
        let target_fp = compute_layout_fingerprint(target_layout);
        let mut best_stage1_match: Option<(&TemplatePair, u32)> = None;

        for template in &self.templates {
            let dist = hamming_distance(target_fp, template.metadata.layout_fingerprint);
            if dist <= self.max_hamming_distance {
                if let Some((_, best_dist)) = best_stage1_match {
                    if dist < best_dist {
                        best_stage1_match = Some((template, dist));
                    }
                } else {
                    best_stage1_match = Some((template, dist));
                }
            }
        }

        if let Some((matched, _)) = best_stage1_match {
            return vec![matched.clone()];
        }

        // --- STAGE 2: Semantic Layout Vector Fallback ---
        if let Some(q_vec) = target_embedding {
            let mut candidate_scores: Vec<(usize, f32)> = Vec::new();

            for (idx, emb_opt) in self.embeddings.iter().enumerate() {
                if let Some(doc_vec) = emb_opt {
                    let sim = cosine_similarity(q_vec, doc_vec);
                    if sim >= self.similarity_threshold {
                        candidate_scores.push((idx, sim));
                    }
                }
            }

            candidate_scores
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            candidate_scores.truncate(self.max_injected);

            if !candidate_scores.is_empty() {
                return candidate_scores
                    .into_iter()
                    .map(|(idx, _)| self.templates[idx].clone())
                    .collect();
            }
        }

        // Zero-shot fallback: return 0 exemplars
        Vec::new()
    }
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
