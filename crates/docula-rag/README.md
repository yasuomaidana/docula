# docula-rag

Layout-aware table chunking, an in-memory vector store, hybrid BM25 + dense retrieval, and `EmbedagGraphRetriever`, which bridges chunks to `embedag_rag::VerifiedRagEngine`. (`oxirag` was removed in DOCULA-46.)

## Key types

`TableAwareChunker`, `DocumentChunk`, `HybridIndex`, `RagRetriever`, `RagConfig`, `EmbedagGraphRetriever`, `DocumentKnowledgeGraph`

## Specification

- [Module 10: RAG, Verified Retrieval & Large Document Retrieval Architecture](../../docs/11_RAG_AND_LARGE_DOCUMENT_RETRIEVAL.md)

## Internal dependencies

`docula-core`, `docula-ocr`, `embedag-rag`

Part of the [docula](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
