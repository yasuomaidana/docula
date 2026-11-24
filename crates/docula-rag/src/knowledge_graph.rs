//! Document Knowledge Graph (DocKG) built on the RDF data model of `oxrdf`.
//!
//! Converts document layouts, entities, and line items into standard RDF
//! triples for semantic reasoning and cross-document reconciliation.

use oxrdf::{Literal, NamedNode, NamedOrBlankNode, Term, Triple};

/// In-memory Document Knowledge Graph of `oxrdf` triples.
#[derive(Debug, Default)]
pub struct DocumentKnowledgeGraph {
    triples: Vec<Triple>,
}

impl DocumentKnowledgeGraph {
    /// Creates an empty document knowledge graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            triples: Vec::new(),
        }
    }

    /// Adds a semantic statement (Subject -> Predicate -> Object) to the knowledge graph.
    pub fn add_fact(&mut self, subject_uri: &str, predicate_uri: &str, object_value: &str) {
        let subject = NamedOrBlankNode::NamedNode(NamedNode::new_unchecked(subject_uri));
        let predicate = NamedNode::new_unchecked(predicate_uri);
        let object = Term::Literal(Literal::new_simple_literal(object_value));

        self.triples.push(Triple::new(subject, predicate, object));
    }

    /// Returns the total number of triples in the document knowledge graph.
    #[must_use]
    pub fn len(&self) -> usize {
        self.triples.len()
    }

    /// Checks if the knowledge graph is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.triples.is_empty()
    }

    /// Finds all facts matching a given subject URI.
    pub fn find_by_subject(&self, subject_uri: &str) -> Vec<(&Triple, String)> {
        let expected_subj = NamedOrBlankNode::NamedNode(NamedNode::new_unchecked(subject_uri));
        self.triples
            .iter()
            .filter(|t| t.subject == expected_subj)
            .map(|t| (t, t.object.to_string()))
            .collect()
    }
}
