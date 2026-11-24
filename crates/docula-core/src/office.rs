use serde::{Deserialize, Serialize};
use std::path::Path;

/// High-level categorization of popular ofimatic and office suite file formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OfficeFormat {
    /// Spreadsheet and tabular data documents (Excel, CSV, TSV, Calc ODS).
    Spreadsheet(SpreadsheetSubtype),
    /// Slide presentation documents (PowerPoint, Impress ODP).
    Presentation(PresentationSubtype),
    /// Word processing and rich text documents (Word DOCX, Writer ODT, RTF, PlainText).
    WordProcessing(WordSubtype),
}

/// Subtypes for spreadsheet and tabular documents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpreadsheetSubtype {
    /// Microsoft Excel OpenXML Spreadsheet (`.xlsx`).
    Xlsx,
    /// Legacy Microsoft Excel Binary (`.xls`).
    Xls,
    /// OpenDocument Spreadsheet (`.ods`).
    Ods,
    /// RFC 4180 Comma-Separated Values (`.csv`).
    Csv,
    /// Tab-Separated Values (`.tsv`).
    Tsv,
}

/// Subtypes for slide presentation documents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PresentationSubtype {
    /// Microsoft PowerPoint OpenXML Presentation (`.pptx`).
    Pptx,
    /// Legacy Microsoft PowerPoint Binary (`.ppt`).
    Ppt,
    /// OpenDocument Presentation (`.odp`).
    Odp,
}

/// Subtypes for word processing and rich text documents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WordSubtype {
    /// Microsoft Word OpenXML Document (`.docx`).
    Docx,
    /// Legacy Microsoft Word Binary (`.doc`).
    Doc,
    /// OpenDocument Text (`.odt`).
    Odt,
    /// Rich Text Format (`.rtf`).
    Rtf,
    /// Plain text document (`.txt`, `.text`).
    PlainText,
}

impl OfficeFormat {
    /// Detects the office format from an optional file path and/or byte header.
    #[must_use]
    pub fn detect(path: Option<&Path>, bytes: Option<&[u8]>) -> Option<Self> {
        if let Some(p) = path
            && let Some(ext) = p.extension().and_then(|e| e.to_str())
            && let Some(fmt) = Self::from_extension(ext)
        {
            return Some(fmt);
        }

        if let Some(b) = bytes {
            Self::detect_from_bytes(b)
        } else {
            None
        }
    }

    /// Maps a file extension string into an [`OfficeFormat`].
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        let ext_lower = ext.to_ascii_lowercase();
        match ext_lower.as_str() {
            "xlsx" | "xlsm" => Some(Self::Spreadsheet(SpreadsheetSubtype::Xlsx)),
            "xls" => Some(Self::Spreadsheet(SpreadsheetSubtype::Xls)),
            "ods" => Some(Self::Spreadsheet(SpreadsheetSubtype::Ods)),
            "csv" => Some(Self::Spreadsheet(SpreadsheetSubtype::Csv)),
            "tsv" => Some(Self::Spreadsheet(SpreadsheetSubtype::Tsv)),
            "pptx" | "pptm" => Some(Self::Presentation(PresentationSubtype::Pptx)),
            "ppt" => Some(Self::Presentation(PresentationSubtype::Ppt)),
            "odp" => Some(Self::Presentation(PresentationSubtype::Odp)),
            "docx" | "docm" => Some(Self::WordProcessing(WordSubtype::Docx)),
            "doc" => Some(Self::WordProcessing(WordSubtype::Doc)),
            "odt" => Some(Self::WordProcessing(WordSubtype::Odt)),
            "rtf" => Some(Self::WordProcessing(WordSubtype::Rtf)),
            "txt" | "text" => Some(Self::WordProcessing(WordSubtype::PlainText)),
            _ => None,
        }
    }

    /// Detects format from binary magic bytes when extension is ambiguous or missing.
    #[must_use]
    pub fn detect_from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"{\\rtf") {
            return Some(Self::WordProcessing(WordSubtype::Rtf));
        }

        if bytes.is_empty() {
            return None;
        }

        // Check if plain ASCII/UTF-8 text with CSV-like delimiters
        if bytes
            .iter()
            .all(|&b| b.is_ascii() && (b >= 0x20 || b == b'\n' || b == b'\r' || b == b'\t'))
        {
            if bytes.contains(&b',') {
                return Some(Self::Spreadsheet(SpreadsheetSubtype::Csv));
            } else if bytes.contains(&b'\t') {
                return Some(Self::Spreadsheet(SpreadsheetSubtype::Tsv));
            }
        }

        None
    }

    /// Returns `true` if this format is any spreadsheet or tabular data format.
    #[must_use]
    pub const fn is_spreadsheet(&self) -> bool {
        matches!(self, Self::Spreadsheet(_))
    }

    /// Returns `true` if this format is a slide presentation format.
    #[must_use]
    pub const fn is_presentation(&self) -> bool {
        matches!(self, Self::Presentation(_))
    }

    /// Returns `true` if this format is a word processing or textual document format.
    #[must_use]
    pub const fn is_word_processing(&self) -> bool {
        matches!(self, Self::WordProcessing(_))
    }

    /// Standard MIME content-type string for this office format.
    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::Spreadsheet(SpreadsheetSubtype::Xlsx) => {
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            }
            Self::Spreadsheet(SpreadsheetSubtype::Xls) => "application/vnd.ms-excel",
            Self::Spreadsheet(SpreadsheetSubtype::Ods) => {
                "application/vnd.oasis.opendocument.spreadsheet"
            }
            Self::Spreadsheet(SpreadsheetSubtype::Csv) => "text/csv",
            Self::Spreadsheet(SpreadsheetSubtype::Tsv) => "text/tab-separated-values",
            Self::Presentation(PresentationSubtype::Pptx) => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            }
            Self::Presentation(PresentationSubtype::Ppt) => "application/vnd.ms-powerpoint",
            Self::Presentation(PresentationSubtype::Odp) => {
                "application/vnd.oasis.opendocument.presentation"
            }
            Self::WordProcessing(WordSubtype::Docx) => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
            Self::WordProcessing(WordSubtype::Doc) => "application/msword",
            Self::WordProcessing(WordSubtype::Odt) => "application/vnd.oasis.opendocument.text",
            Self::WordProcessing(WordSubtype::Rtf) => "application/rtf",
            Self::WordProcessing(WordSubtype::PlainText) => "text/plain",
        }
    }
}
