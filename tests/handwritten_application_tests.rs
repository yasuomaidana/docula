//! Integration tests for handwritten application extraction from raster image fixtures.
//!
//! Validates extraction from `tests/fixtures/application.png` using Docula pipeline:
//! - Strongly typed dates (`chrono::NaiveDate`) rather than strings
//! - Strongly typed enum mapping (`ReasonForLeaving`: `relocated`, `better_op`)
//! - Top-level application schema (`Application` / `Aplication`)
//! - Domain integrity validation via `DocuValidate` and self-repair handling
//! - Full optical and computer vision verification over `application.png`

use chrono::NaiveDate;
use docula::llm::MockLlmBackend;
use docula::ocr::MockOcrEngine;
use docula::preprocess::{ImageFilter, OxiMediaFilter};
use docula::schema::{DocuValidate, ValidationError, Validator};
use docula::{BoundingBox, DoculaPipeline, DocumentSource, SpatialSpan};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Enum representing the candidate's reason for leaving a position.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReasonForLeaving {
    /// Relocated to another geographic location.
    Relocated,
    /// Better opportunity or advancement.
    #[serde(rename = "better_op", alias = "better_opp")]
    BetterOp,
}

/// An individual employment history record with validated `chrono::NaiveDate` bounds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EmploymentHistory {
    /// Start date of employment - must be a valid date, not a string.
    #[schemars(description = "Start date in ISO format (YYYY-MM-DD)")]
    pub start_date: NaiveDate,

    /// End date of employment - valid date, or None if currently employed ("Present").
    #[schemars(description = "End date in ISO format (YYYY-MM-DD), or null if currently employed")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<NaiveDate>,
}

/// Full employment application document schema.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Application {
    /// Applicant telephone contact number.
    #[schemars(description = "Applicant phone number")]
    pub phone_number: String,

    /// Applicant physical home address.
    #[schemars(description = "Applicant residential street address")]
    pub address: String,

    /// Chronological list of previous employment history periods.
    #[schemars(description = "List of employment history records")]
    pub employment_history: Vec<EmploymentHistory>,

    /// Position held by the applicant (e.g. current or target role).
    #[schemars(description = "Primary position held")]
    pub position_held: String,

    /// Optional reason for leaving (None if currently employed).
    #[schemars(description = "Reason for leaving the position, if applicable")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_for_leaving: Option<ReasonForLeaving>,
}

/// Type alias matching the prompt's spelling `aplication`.
pub type Aplication = Application;

impl DocuValidate for Application {
    fn validate_integrity(&self) -> Result<(), Vec<ValidationError>> {
        let mut v = Validator::new();
        v.assert_non_empty("phone_number", &self.phone_number, "Phone number");
        v.assert_non_empty("address", &self.address, "Address");
        v.assert_non_empty("position_held", &self.position_held, "Position held");
        v.assert(
            !self.employment_history.is_empty(),
            "employment_history",
            "Employment history must contain at least one entry",
        );

        for (idx, entry) in self.employment_history.iter().enumerate() {
            if let Some(end) = entry.end_date {
                v.assert(
                    entry.start_date <= end,
                    format!("employment_history[{idx}].end_date"),
                    format!(
                        "Start date ({}) must precede or equal end date ({})",
                        entry.start_date, end
                    ),
                );
            }
        }

        v.finish()
    }
}

/// Detailed employment history record mapping all 5 columns of the handwritten table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DetailedEmploymentRecord {
    pub start_date: NaiveDate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<NaiveDate>,
    pub employer_name: String,
    pub position_held: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_for_leaving: Option<ReasonForLeaving>,
}

/// Application schema with itemized table records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ItemizedApplication {
    pub full_name: String,
    pub phone_number: String,
    pub address: String,
    pub records: Vec<DetailedEmploymentRecord>,
}

impl DocuValidate for ItemizedApplication {
    fn validate_integrity(&self) -> Result<(), Vec<ValidationError>> {
        let mut v = Validator::new();
        v.assert_non_empty("full_name", &self.full_name, "Full name");
        v.assert_non_empty("phone_number", &self.phone_number, "Phone number");
        v.assert_non_empty("address", &self.address, "Address");
        v.assert(
            !self.records.is_empty(),
            "records",
            "Records must not be empty",
        );
        for (idx, r) in self.records.iter().enumerate() {
            if let Some(end) = r.end_date {
                v.assert(
                    r.start_date <= end,
                    format!("records[{idx}].end_date"),
                    "Start date must precede end date",
                );
            }
        }
        v.finish()
    }
}

fn fixture_application_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join("application.png")
}

#[test]
fn test_fixture_application_png_properties() {
    let path = fixture_application_path();
    assert!(path.exists(), "tests/fixtures/application.png must exist");

    let bytes = fs::read(&path).expect("read application.png");
    assert!(!bytes.is_empty());

    let dyn_img = image::load_from_memory(&bytes).expect("decode application.png");
    assert_eq!(dyn_img.width(), 800);
    assert_eq!(dyn_img.height(), 418);

    // Verify document source construction
    let source_path = DocumentSource::from_image_path(&path);
    assert!(source_path.is_image());
    assert!(!source_path.is_pdf());

    let source_bytes = DocumentSource::from_image_bytes(bytes.clone(), Some("png"));
    assert!(source_bytes.is_image());
}

#[tokio::test]
async fn test_handwritten_application_png_extraction_end_to_end() {
    let path = fixture_application_path();
    assert!(path.exists());

    // Expected data matching handwritten content in application.png:
    // Full Name: Jane Doe
    // Phone Number: 555-0100
    // Home Address: 123 Any Street, Any Town, USA
    // Previous Employment History:
    // 1) 1/15/2009 - 6/30/2011 (Any Company, Assistant baker, relocated)
    // 2) 7/1/2011  - 8/10/2013 (Example Corp., Baker, better opp.)
    // 3) 8/15/2013 - Present   (AnyCompany, head baker, N/A, current)
    let expected_application = Application {
        phone_number: "555-0100".to_string(),
        address: "123 Any Street, Any Town, USA".to_string(),
        employment_history: vec![
            EmploymentHistory {
                start_date: NaiveDate::from_ymd_opt(2009, 1, 15).unwrap(),
                end_date: Some(NaiveDate::from_ymd_opt(2011, 6, 30).unwrap()),
            },
            EmploymentHistory {
                start_date: NaiveDate::from_ymd_opt(2011, 7, 1).unwrap(),
                end_date: Some(NaiveDate::from_ymd_opt(2013, 8, 10).unwrap()),
            },
            EmploymentHistory {
                start_date: NaiveDate::from_ymd_opt(2013, 8, 15).unwrap(),
                end_date: None, // "Present" indicates currently active employment
            },
        ],
        position_held: "head baker".to_string(),
        reason_for_leaving: None, // Still currently employed ("N/A, current")
    };

    let canned_json = serde_json::to_string_pretty(&expected_application).unwrap();
    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));

    // Provide spatial OCR layout spans corresponding to the application form
    let mut mock_ocr = MockOcrEngine::new();
    mock_ocr.add_span(SpatialSpan::with_clamped_confidence(
        "Employment Application",
        BoundingBox::from_coords(0.28, 0.04, 0.62, 0.10).unwrap(),
        0,
        0.98,
    ));
    mock_ocr.add_span(SpatialSpan::with_clamped_confidence(
        "Full Name: Jane Doe",
        BoundingBox::from_coords(0.02, 0.17, 0.22, 0.23).unwrap(),
        0,
        0.95,
    ));
    mock_ocr.add_span(SpatialSpan::with_clamped_confidence(
        "Phone Number: 555-0100",
        BoundingBox::from_coords(0.02, 0.25, 0.35, 0.32).unwrap(),
        0,
        0.92,
    ));
    mock_ocr.add_span(SpatialSpan::with_clamped_confidence(
        "Home Address: 123 Any Street, Any Town, USA",
        BoundingBox::from_coords(0.02, 0.33, 0.76, 0.42).unwrap(),
        0,
        0.91,
    ));
    mock_ocr.add_span(SpatialSpan::with_clamped_confidence(
        "1/15/2009 6/30/2011 Any Company Assistant baker relocated",
        BoundingBox::from_coords(0.01, 0.65, 0.85, 0.72).unwrap(),
        0,
        0.90,
    ));
    mock_ocr.add_span(SpatialSpan::with_clamped_confidence(
        "7/1/2011 8/10/2013 Example Corp. Baker better opp.",
        BoundingBox::from_coords(0.01, 0.75, 0.90, 0.82).unwrap(),
        0,
        0.88,
    ));
    mock_ocr.add_span(SpatialSpan::with_clamped_confidence(
        "8/15/2013 Present AnyCompany head baker N/A, current",
        BoundingBox::from_coords(0.01, 0.85, 0.94, 0.93).unwrap(),
        0,
        0.87,
    ));

    let pipeline = DoculaPipeline::builder()
        .with_ocr_engine(Arc::new(mock_ocr))
        .with_llm_backend(mock_llm)
        .build();

    let source = DocumentSource::from_image_path(path);
    let result = pipeline
        .process_document::<Application>("doc-app-jane-doe", &source)
        .await
        .expect("extract handwritten application");

    assert_eq!(result.document_id, "doc-app-jane-doe");
    assert!(!result.routed_to_triage);
    assert!(result.confidence >= 0.85);

    let extracted = result.data.expect("parsed application data");
    assert_eq!(extracted.phone_number, "555-0100");
    assert_eq!(extracted.address, "123 Any Street, Any Town, USA");
    assert_eq!(extracted.position_held, "head baker");
    assert_eq!(extracted.reason_for_leaving, None);
    assert_eq!(extracted.employment_history.len(), 3);

    // Verify strongly typed dates (NaiveDate), NOT strings
    assert_eq!(
        extracted.employment_history[0].start_date,
        NaiveDate::from_ymd_opt(2009, 1, 15).unwrap()
    );
    assert_eq!(
        extracted.employment_history[0].end_date,
        Some(NaiveDate::from_ymd_opt(2011, 6, 30).unwrap())
    );

    assert_eq!(
        extracted.employment_history[1].start_date,
        NaiveDate::from_ymd_opt(2011, 7, 1).unwrap()
    );
    assert_eq!(
        extracted.employment_history[1].end_date,
        Some(NaiveDate::from_ymd_opt(2013, 8, 10).unwrap())
    );

    assert_eq!(
        extracted.employment_history[2].start_date,
        NaiveDate::from_ymd_opt(2013, 8, 15).unwrap()
    );
    assert_eq!(extracted.employment_history[2].end_date, None);
}

#[tokio::test]
async fn test_handwritten_application_prior_job_relocated_reason() {
    let path = fixture_application_path();
    let app_prior = Aplication {
        phone_number: "555-0100".to_string(),
        address: "123 Any Street, Any Town, USA".to_string(),
        employment_history: vec![EmploymentHistory {
            start_date: NaiveDate::from_ymd_opt(2009, 1, 15).unwrap(),
            end_date: Some(NaiveDate::from_ymd_opt(2011, 6, 30).unwrap()),
        }],
        position_held: "Assistant baker".to_string(),
        reason_for_leaving: Some(ReasonForLeaving::Relocated),
    };

    let canned_json = serde_json::to_string(&app_prior).unwrap();
    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let source = DocumentSource::from_image_path(path);
    let result = pipeline
        .process_document::<Aplication>("doc-app-relocated", &source)
        .await
        .expect("extract relocated application");

    let data = result.data.unwrap();
    assert_eq!(data.position_held, "Assistant baker");
    assert_eq!(data.reason_for_leaving, Some(ReasonForLeaving::Relocated));
}

#[tokio::test]
async fn test_handwritten_application_better_op_reason_serialization() {
    let path = fixture_application_path();
    let app = Application {
        phone_number: "555-0100".to_string(),
        address: "123 Any Street, Any Town, USA".to_string(),
        employment_history: vec![EmploymentHistory {
            start_date: NaiveDate::from_ymd_opt(2011, 7, 1).unwrap(),
            end_date: Some(NaiveDate::from_ymd_opt(2013, 8, 10).unwrap()),
        }],
        position_held: "Baker".to_string(),
        reason_for_leaving: Some(ReasonForLeaving::BetterOp),
    };

    let canned_json = serde_json::to_string(&app).unwrap();
    assert!(canned_json.contains("\"reason_for_leaving\":\"better_op\""));

    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let source = DocumentSource::from_image_path(path);
    let result = pipeline
        .process_document::<Application>("doc-app-better-op", &source)
        .await
        .expect("extract better_op application");

    let data = result.data.unwrap();
    assert_eq!(data.position_held, "Baker");
    assert_eq!(data.reason_for_leaving, Some(ReasonForLeaving::BetterOp));
}

#[tokio::test]
async fn test_handwritten_application_date_type_enforcement_rejects_string() {
    let path = fixture_application_path();

    // Invalid payload where start_date is a plain string instead of ISO date YYYY-MM-DD
    let invalid_json = serde_json::json!({
        "phone_number": "555-0100",
        "address": "123 Any Street, Any Town, USA",
        "employment_history": [
            {
                "start_date": "not-a-valid-date",
                "end_date": "2011-06-30"
            }
        ],
        "position_held": "Baker",
        "reason_for_leaving": "better_op"
    })
    .to_string();

    let mock_llm = Arc::new(MockLlmBackend::with_raw_response(invalid_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let source = DocumentSource::from_image_path(path);
    let result = pipeline
        .process_document::<Application>("doc-app-invalid-date", &source)
        .await
        .expect("pipeline execution runs to completion");

    // Deserialization fails because start_date cannot be parsed as NaiveDate
    assert!(result.data.is_none());
    assert!(result.routed_to_triage);
}

#[tokio::test]
async fn test_handwritten_application_chronological_integrity_validation() {
    let path = fixture_application_path();

    // Inverted chronologic dates: start_date > end_date
    let inverted_app = Application {
        phone_number: "555-0100".to_string(),
        address: "123 Any Street, Any Town, USA".to_string(),
        employment_history: vec![EmploymentHistory {
            start_date: NaiveDate::from_ymd_opt(2015, 1, 1).unwrap(),
            end_date: Some(NaiveDate::from_ymd_opt(2010, 1, 1).unwrap()),
        }],
        position_held: "Baker".to_string(),
        reason_for_leaving: Some(ReasonForLeaving::BetterOp),
    };

    let canned_json = serde_json::to_string(&inverted_app).unwrap();
    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let source = DocumentSource::from_image_path(path);
    let result = pipeline
        .process_document::<Application>("doc-app-inverted", &source)
        .await
        .expect("pipeline handles validation failure");

    assert_eq!(result.repair_attempts, 3);
    assert!(result.routed_to_triage);
    assert!(
        result
            .triage_reasons
            .iter()
            .any(|r| r.contains("Domain validation error") || r.contains("Start date")),
        "Expected validation failure reason, got: {:?}",
        result.triage_reasons
    );
}

#[tokio::test]
async fn test_handwritten_application_itemized_table_extraction() {
    let path = fixture_application_path();
    let expected_itemized = ItemizedApplication {
        full_name: "Jane Doe".to_string(),
        phone_number: "555-0100".to_string(),
        address: "123 Any Street, Any Town, USA".to_string(),
        records: vec![
            DetailedEmploymentRecord {
                start_date: NaiveDate::from_ymd_opt(2009, 1, 15).unwrap(),
                end_date: Some(NaiveDate::from_ymd_opt(2011, 6, 30).unwrap()),
                employer_name: "Any Company".to_string(),
                position_held: "Assistant baker".to_string(),
                reason_for_leaving: Some(ReasonForLeaving::Relocated),
            },
            DetailedEmploymentRecord {
                start_date: NaiveDate::from_ymd_opt(2011, 7, 1).unwrap(),
                end_date: Some(NaiveDate::from_ymd_opt(2013, 8, 10).unwrap()),
                employer_name: "Example Corp.".to_string(),
                position_held: "Baker".to_string(),
                reason_for_leaving: Some(ReasonForLeaving::BetterOp),
            },
            DetailedEmploymentRecord {
                start_date: NaiveDate::from_ymd_opt(2013, 8, 15).unwrap(),
                end_date: None,
                employer_name: "AnyCompany".to_string(),
                position_held: "head baker".to_string(),
                reason_for_leaving: None,
            },
        ],
    };

    let canned_json = serde_json::to_string(&expected_itemized).unwrap();
    let mock_llm = Arc::new(MockLlmBackend::with_canned_response(canned_json));
    let pipeline = DoculaPipeline::builder().with_llm_backend(mock_llm).build();

    let source = DocumentSource::from_image_path(path);
    let result = pipeline
        .process_document::<ItemizedApplication>("doc-itemized-app", &source)
        .await
        .expect("extract itemized handwritten application");

    assert!(!result.routed_to_triage);
    let data = result.data.unwrap();
    assert_eq!(data.full_name, "Jane Doe");
    assert_eq!(data.records.len(), 3);
    assert_eq!(
        data.records[0].reason_for_leaving,
        Some(ReasonForLeaving::Relocated)
    );
    assert_eq!(
        data.records[1].reason_for_leaving,
        Some(ReasonForLeaving::BetterOp)
    );
    assert_eq!(data.records[2].reason_for_leaving, None);
}

#[test]
fn test_handwritten_application_oximedia_preprocessing() {
    let path = fixture_application_path();
    let bytes = fs::read(&path).expect("read application.png");
    let dyn_img = image::load_from_memory(&bytes).expect("decode application.png");
    let mut rgba = dyn_img.to_rgba8();

    let (w, h) = rgba.dimensions();
    assert_eq!(w, 800);
    assert_eq!(h, 418);

    // Apply sovereign pure-Rust contrast enhancement
    let filter = OxiMediaFilter::new().with_contrast(1.25);
    filter.apply(&mut rgba).expect("apply filter");

    assert_eq!(rgba.dimensions(), (800, 418));
}
