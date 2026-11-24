//! # docula-rag
//!
//! Layout-aware table chunking, in-memory vector indexing, and hybrid BM25 retrieval
//! for large document packets in the `docula` document intelligence engine.
//!
//! - [`EmbedagGraphRetriever`]: Hybrid retrieval and verified querying via `embedag_rag::VerifiedRagEngine`.
//! - [`DocumentKnowledgeGraph`]: in-memory RDF knowledge graph built on `oxrdf`.

pub mod chunk;
#[cfg(feature = "embedag-integration")]
pub mod embedag_adapter;
pub mod error;
pub mod index;
pub mod knowledge_graph;
pub mod retriever;

pub use chunk::{DocumentChunk, TableAwareChunker};
#[cfg(feature = "embedag-integration")]
pub use embedag_adapter::EmbedagGraphRetriever;
pub use error::RagError;
pub use index::HybridIndex;
pub use knowledge_graph::DocumentKnowledgeGraph;
pub use retriever::{RagConfig, RagRetriever};
