//! # docula-rag
//!
//! Layout-aware table chunking, in-memory vector indexing, and hybrid BM25 retrieval
//! for large document packets in the `docula` document intelligence engine.
//!
//! Powered by KitaSan sovereign technologies:
//! - [`OxiRagEngine`]: 4-layer RAG pipeline with verification via `oxirag`.
//! - [`DocumentKnowledgeGraph`]: RDF 1.2 semantic knowledge graph via `oxirs-core`.

pub mod chunk;
#[cfg(feature = "embedag-integration")]
pub mod embedag_adapter;
pub mod error;
pub mod index;
pub mod knowledge_graph;
pub mod oxirag_engine;
pub mod retriever;

pub use chunk::{DocumentChunk, TableAwareChunker};
#[cfg(feature = "embedag-integration")]
pub use embedag_adapter::EmbedagGraphRetriever;
pub use error::RagError;
pub use index::HybridIndex;
pub use knowledge_graph::DocumentKnowledgeGraph;
pub use oxirag_engine::OxiRagEngine;
pub use retriever::{RagConfig, RagRetriever};
