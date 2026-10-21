use docula_ocr::SpatialLayoutMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Metadata identifying a curated document exemplar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExemplarMetadata {
    /// Unique identifier (e.g. "pge_electric_bill_v1").
    pub exemplar_id: String,
    /// Optional vendor/merchant name.
    pub vendor_name: Option<String>,
    /// Optional vendor tax ID / VAT number.
    pub vendor_tax_id: Option<String>,
    /// Document category (e.g. "electricity_bill", "invoice", "customs_declaration").
    pub document_type: String,
    /// 64-bit SimHash geometric layout fingerprint.
    pub layout_fingerprint: u64,
    /// Creation timestamp (Unix epoch seconds).
    pub created_at_unix: u64,
}

/// A registered exemplar pairing a spatial OCR layout with its expected ground truth JSON payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplatePair {
    /// Identifying metadata.
    pub metadata: ExemplarMetadata,
    /// Optional path to source document file.
    pub source_artifact_path: Option<PathBuf>,
    /// Ground-truth spatial layout map.
    pub spatial_layout: SpatialLayoutMap,
    /// Expected ground-truth JSON value.
    pub ground_truth_json: serde_json::Value,
}
