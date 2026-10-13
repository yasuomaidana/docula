use docula_core::{BoundingBox, SpatialSpan};
use docula_ocr::SpatialLayoutMap;
use docula_rag::{DocumentChunk, HybridIndex, RagConfig, RagRetriever, TableAwareChunker};

#[test]
fn test_table_aware_chunking_keeps_table_intact() {
    // Page with a header line, then a 3-row table with 3 columns, then a footer line
    let span_header = SpatialSpan::new(
        "Annual Financial Audit Summary 2026".to_string(),
        BoundingBox::from_coords_clamped(0.1, 0.05, 0.9, 0.08),
        0,
        1.0,
    )
    .unwrap();

    // Table rows (each with 3 columns)
    let r0_c0 = SpatialSpan::new(
        "Q1 Revenue".to_string(),
        BoundingBox::from_coords_clamped(0.1, 0.20, 0.3, 0.23),
        0,
        1.0,
    )
    .unwrap();
    let r0_c1 = SpatialSpan::new(
        "$1,200,000".to_string(),
        BoundingBox::from_coords_clamped(0.4, 0.20, 0.6, 0.23),
        0,
        1.0,
    )
    .unwrap();
    let r0_c2 = SpatialSpan::new(
        "+12%".to_string(),
        BoundingBox::from_coords_clamped(0.7, 0.20, 0.8, 0.23),
        0,
        1.0,
    )
    .unwrap();

    let r1_c0 = SpatialSpan::new(
        "Q2 Revenue".to_string(),
        BoundingBox::from_coords_clamped(0.1, 0.25, 0.3, 0.28),
        0,
        1.0,
    )
    .unwrap();
    let r1_c1 = SpatialSpan::new(
        "$1,450,000".to_string(),
        BoundingBox::from_coords_clamped(0.4, 0.25, 0.6, 0.28),
        0,
        1.0,
    )
    .unwrap();
    let r1_c2 = SpatialSpan::new(
        "+18%".to_string(),
        BoundingBox::from_coords_clamped(0.7, 0.25, 0.8, 0.28),
        0,
        1.0,
    )
    .unwrap();

    let span_footer = SpatialSpan::new(
        "Prepared by Certified Public Accountants LLP".to_string(),
        BoundingBox::from_coords_clamped(0.1, 0.85, 0.9, 0.88),
        0,
        1.0,
    )
    .unwrap();

    let spans = vec![
        span_header,
        r0_c0,
        r0_c1,
        r0_c2,
        r1_c0,
        r1_c1,
        r1_c2,
        span_footer,
    ];
    let layout = SpatialLayoutMap::from_spans(spans, (1000, 1000));

    let chunker = TableAwareChunker::new(500);
    let chunks = chunker.chunk_layout(0, &layout, 0);

    // Should create 3 chunks: Header section, Table Grid, Footer section
    assert_eq!(chunks.len(), 3);
    assert!(!chunks[0].is_table);
    assert!(chunks[0].text.contains("Audit Summary"));

    assert!(chunks[1].is_table);
    assert!(chunks[1].text.contains("Table Grid"));
    assert!(chunks[1].text.contains("Q1 Revenue"));
    assert!(chunks[1].text.contains("Q2 Revenue"));

    assert!(!chunks[2].is_table);
    assert!(chunks[2].text.contains("Accountants"));
}

#[test]
fn test_hybrid_index_bm25_and_vector() {
    let mut index = HybridIndex::new();

    let chunk1 = DocumentChunk {
        id: 0,
        page_index: 0,
        text: "Supplier invoice Acme Global Tools widgets payment".to_string(),
        bbox: BoundingBox::from_coords_clamped(0.0, 0.0, 1.0, 0.3),
        is_table: false,
        token_estimate: 8,
    };
    let emb1 = vec![0.9, 0.1, 0.0, 0.0];

    let chunk2 = DocumentChunk {
        id: 1,
        page_index: 1,
        text: "Employee travel expenses hotel lodging flight tickets".to_string(),
        bbox: BoundingBox::from_coords_clamped(0.0, 0.0, 1.0, 0.5),
        is_table: false,
        token_estimate: 8,
    };
    let emb2 = vec![0.0, 0.0, 0.9, 0.2];

    index.add_chunk(chunk1, Some(emb1));
    index.add_chunk(chunk2, Some(emb2));

    // Keyword search for "widgets"
    let results_kw = index.query("widgets", None, 1, 0.0);
    assert_eq!(results_kw.len(), 1);
    assert_eq!(results_kw[0].0.id, 0);

    // Vector query close to emb2
    let q_vec = vec![0.1, 0.0, 0.8, 0.1];
    let results_vec = index.query("flight expenses", Some(&q_vec), 1, 0.7);
    assert_eq!(results_vec.len(), 1);
    assert_eq!(results_vec[0].0.id, 1);
}

#[test]
fn test_empty_chunks_handled_gracefully() {
    let layout = SpatialLayoutMap::from_spans(Vec::new(), (800, 600));
    let chunker = TableAwareChunker::default();
    let chunks = chunker.chunk_layout(0, &layout, 0);
    assert!(chunks.is_empty());

    let retriever = RagRetriever::new(RagConfig::default());
    let results = retriever.retrieve("query", None);
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_oxirag_engine_query() {
    use docula_rag::OxiRagEngine;

    let engine = OxiRagEngine::new(384);
    let corpus = [
        "Invoice #10023 issued by Acme Corp on 2026-10-10.",
        "Total amount payable is ,250.00 within 30 days.",
    ];
    let ans = engine.query_verified(&corpus, "What is the invoice number?").await.expect("oxirag success");
    assert!(!ans.is_empty());
}

#[test]
fn test_oxirs_knowledge_graph() {
    use docula_rag::DocumentKnowledgeGraph;

    let mut kg = DocumentKnowledgeGraph::new();
    kg.add_fact("http://example.org/invoice/101", "http://example.org/vendor", "Acme Corp");
    kg.add_fact("http://example.org/invoice/101", "http://example.org/total", "500.00");

    assert_eq!(kg.len(), 2);
    let facts = kg.find_by_subject("http://example.org/invoice/101");
    assert_eq!(facts.len(), 2);
}
