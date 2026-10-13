use crate::dfa::SchemaDfa;

/// Autoregressive token logits processor enforcing JSON Schema grammar constraints.
#[derive(Clone, Debug)]
pub struct DfaLogitsMasker {
    dfa: SchemaDfa,
}

impl DfaLogitsMasker {
    /// Creates a new logits masker wrapping a compiled schema DFA.
    #[must_use]
    pub fn new(dfa: SchemaDfa) -> Self {
        Self { dfa }
    }

    /// Access reference to underlying compiled DFA.
    #[must_use]
    pub fn dfa(&self) -> &SchemaDfa {
        &self.dfa
    }

    /// Intercepts raw unnormalized logit scores across the vocabulary and masks invalid tokens to -infinity.
    ///
    /// Tokens that would create an invalid prefix according to the grammar receive `f32::NEG_INFINITY`.
    /// Valid transitions retain their original raw logit scores.
    pub fn mask_logits(&self, current_prefix: &str, logits: &mut [f32], vocab: &[String]) {
        let count = logits.len().min(vocab.len());
        for (i, token) in vocab.iter().take(count).enumerate() {
            if !self.dfa.can_transition(current_prefix, token) {
                logits[i] = f32::NEG_INFINITY;
            }
        }
    }

    /// Returns the indices of all vocabulary tokens that represent valid grammatical transitions.
    #[must_use]
    pub fn valid_token_indices(&self, current_prefix: &str, vocab: &[String]) -> Vec<usize> {
        vocab
            .iter()
            .enumerate()
            .filter(|(_, token)| self.dfa.can_transition(current_prefix, token))
            .map(|(i, _)| i)
            .collect()
    }

    /// Selects the highest scoring valid token index using greedy decoding.
    #[must_use]
    pub fn sample_greedy(
        &self,
        current_prefix: &str,
        logits: &[f32],
        vocab: &[String],
    ) -> Option<usize> {
        let mut masked = logits.to_vec();
        self.mask_logits(current_prefix, &mut masked, vocab);

        masked
            .iter()
            .enumerate()
            .filter(|&(_, logit)| *logit > f32::NEG_INFINITY)
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
    }
}
