pub mod presentation;
pub mod spreadsheet;
pub mod word;

pub use presentation::{PresentationDeck, PresentationReader, PresentationSlide};
pub use spreadsheet::{SpreadsheetCell, SpreadsheetReader, SpreadsheetSheet, SpreadsheetWorkbook};
pub use word::{WordDocument, WordProcessingReader};

use crate::error::IngestError;
use crate::types::DocumentRoutingReport;
use docula_core::{DocumentSource, OfficeFormat};
use std::fs;

/// Central orchestrator for inspecting and routing ofimatic and office suite documents.
pub struct OfficeInspector;

impl OfficeInspector {
    /// Ingests and routes raw bytes according to the specified [`OfficeFormat`].
    pub fn inspect_bytes(
        format: OfficeFormat,
        bytes: &[u8],
    ) -> Result<DocumentRoutingReport, IngestError> {
        let routes = match format {
            OfficeFormat::Spreadsheet(subtype) => {
                let workbook = SpreadsheetReader::read_bytes(subtype, bytes)?;
                workbook.to_page_routes()
            }
            OfficeFormat::Presentation(subtype) => {
                let deck = PresentationReader::read_bytes(subtype, bytes)?;
                deck.to_page_routes()
            }
            OfficeFormat::WordProcessing(subtype) => {
                let doc = WordProcessingReader::read_bytes(subtype, bytes)?;
                doc.to_page_routes()
            }
        };

        Ok(DocumentRoutingReport::new(routes))
    }

    /// Evaluates a [`DocumentSource`] representing an office document.
    pub fn inspect_source(source: &DocumentSource) -> Result<DocumentRoutingReport, IngestError> {
        match source {
            DocumentSource::OfficeBytes { format, data } => Self::inspect_bytes(*format, data),
            DocumentSource::OfficePath { path, format } => {
                let bytes = fs::read(path)?;
                Self::inspect_bytes(*format, &bytes)
            }
            _ => Err(IngestError::OfficeFormat(
                "Source is not an office document variant".into(),
            )),
        }
    }
}
