use crate::error::IngestError;
use crate::types::PageRoute;
use docula_core::{BoundingBox, SpatialSpan, WordSubtype};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::io::{Cursor, Read};
use zip::ZipArchive;

/// Extracted textual document partitioned into paginated sections.
#[derive(Clone, Debug, PartialEq)]
pub struct WordDocument {
    /// Pages of extracted spatial text spans.
    pub pages: Vec<Vec<SpatialSpan>>,
}

impl WordDocument {
    /// Converts word document pages into born-digital page routes.
    #[must_use]
    pub fn to_page_routes(&self) -> Vec<PageRoute> {
        self.pages
            .iter()
            .enumerate()
            .map(|(idx, page_spans)| PageRoute::BornDigital {
                page_index: idx,
                text_layer: page_spans.clone(),
            })
            .collect()
    }
}

/// Pure-Rust parser for word processing and rich/plain text documents.
pub struct WordProcessingReader;

impl WordProcessingReader {
    /// Ingests document bytes based on subtype into a [`WordDocument`].
    pub fn read_bytes(subtype: WordSubtype, bytes: &[u8]) -> Result<WordDocument, IngestError> {
        if bytes.is_empty() {
            return Err(IngestError::EmptyPayload);
        }

        match subtype {
            WordSubtype::Docx => Self::read_docx(bytes),
            WordSubtype::Odt => Self::read_odt(bytes),
            WordSubtype::PlainText => Self::read_plain_text(bytes),
            WordSubtype::Rtf => Self::read_rtf(bytes),
            WordSubtype::Doc => Err(IngestError::OfficeFormat(
                "Legacy binary .doc format is deprecated; please convert to .docx or .pdf".into(),
            )),
        }
    }

    /// Parses Microsoft Word OpenXML (`.docx`) ZIP streams.
    pub fn read_docx(bytes: &[u8]) -> Result<WordDocument, IngestError> {
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| IngestError::OfficeFormat(format!("Failed to open DOCX zip: {e}")))?;

        let mut doc_file = archive.by_name("word/document.xml").map_err(|e| {
            IngestError::OfficeFormat(format!("DOCX missing word/document.xml: {e}"))
        })?;

        let mut xml_data = String::new();
        doc_file
            .read_to_string(&mut xml_data)
            .map_err(|e| IngestError::OfficeFormat(e.to_string()))?;

        parse_docx_xml(&xml_data)
    }

    /// Parses OpenDocument Text (`.odt`) ZIP streams.
    pub fn read_odt(bytes: &[u8]) -> Result<WordDocument, IngestError> {
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| IngestError::OfficeFormat(format!("Failed to open ODT zip: {e}")))?;

        let mut doc_file = archive
            .by_name("content.xml")
            .map_err(|e| IngestError::OfficeFormat(format!("ODT missing content.xml: {e}")))?;

        let mut xml_data = String::new();
        doc_file
            .read_to_string(&mut xml_data)
            .map_err(|e| IngestError::OfficeFormat(e.to_string()))?;

        parse_odt_xml(&xml_data)
    }

    /// Parses plain text bytes into paginated spatial spans.
    pub fn read_plain_text(bytes: &[u8]) -> Result<WordDocument, IngestError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|e| IngestError::OfficeFormat(format!("Invalid UTF-8 in plain text: {e}")))?;

        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        paginate_lines(&lines)
    }

    /// Parses rich text format (RTF) byte streams by stripping control tags.
    pub fn read_rtf(bytes: &[u8]) -> Result<WordDocument, IngestError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|e| IngestError::OfficeFormat(format!("Invalid UTF-8 in RTF payload: {e}")))?;

        let stripped = strip_rtf_tags(text);
        let lines: Vec<&str> = stripped
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        paginate_lines(&lines)
    }
}

/// Parses DOCX XML into paginated paragraphs.
fn parse_docx_xml(xml: &str) -> Result<WordDocument, IngestError> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut paragraphs = Vec::new();
    let mut current_para = String::new();
    let mut in_text = false;

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(ref e) if e.name().as_ref() == "w:t" => {
                in_text = true;
            }
            Event::End(ref e) if e.name().as_ref() == "w:t" => {
                in_text = false;
            }
            Event::End(ref e) if e.name().as_ref() == "w:p" => {
                let trimmed = current_para.trim();
                if !trimmed.is_empty() {
                    paragraphs.push(trimmed.to_string());
                }
                current_para.clear();
            }
            Event::Text(ref e) if in_text => {
                current_para.push_str(e.as_ref());
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let line_refs: Vec<&str> = paragraphs.iter().map(String::as_str).collect();
    paginate_lines(&line_refs)
}

/// Parses ODT XML into paginated paragraphs.
fn parse_odt_xml(xml: &str) -> Result<WordDocument, IngestError> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut paragraphs = Vec::new();
    let mut current_para = String::new();
    let mut in_text = false;

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(ref e)
                if e.name().as_ref() == "text:p" || e.name().as_ref() == "text:h" =>
            {
                in_text = true;
                current_para.clear();
            }
            Event::End(ref e) if e.name().as_ref() == "text:p" || e.name().as_ref() == "text:h" => {
                in_text = false;
                let trimmed = current_para.trim();
                if !trimmed.is_empty() {
                    paragraphs.push(trimmed.to_string());
                }
            }
            Event::Text(ref e) if in_text => {
                current_para.push_str(e.as_ref());
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let line_refs: Vec<&str> = paragraphs.iter().map(String::as_str).collect();
    paginate_lines(&line_refs)
}

/// Chunks sequential text lines into pages of normalized spatial spans.
fn paginate_lines(lines: &[&str]) -> Result<WordDocument, IngestError> {
    let lines_per_page = 40;
    let mut pages = Vec::new();

    if lines.is_empty() {
        pages.push(Vec::new());
        return Ok(WordDocument { pages });
    }

    for (page_idx, chunk) in lines.chunks(lines_per_page).enumerate() {
        let count = chunk.len().max(1) as f32;
        let step = 0.90 / count;
        let mut page_spans = Vec::with_capacity(chunk.len());

        for (i, &line) in chunk.iter().enumerate() {
            let y0 = (0.05 + i as f32 * step).clamp(0.0, 1.0);
            let y1 = (y0 + step * 0.85).clamp(0.0, 1.0);

            let bbox = BoundingBox::from_coords_clamped(0.05, y0, 0.95, y1);
            page_spans.push(SpatialSpan {
                text: line.to_string(),
                bbox,
                page_index: page_idx,
                confidence: 1.0,
            });
        }
        pages.push(page_spans);
    }

    Ok(WordDocument { pages })
}

/// Strips standard RTF markup commands and brace blocks.
fn strip_rtf_tags(rtf: &str) -> String {
    let mut result = String::with_capacity(rtf.len());
    let mut in_control = false;

    for ch in rtf.chars() {
        if ch == '\\' {
            in_control = true;
        } else if in_control {
            if ch.is_whitespace() || ch == ';' || ch == '\\' || ch == '{' || ch == '}' {
                in_control = false;
                if ch.is_whitespace() {
                    result.push(' ');
                }
            }
        } else if ch != '{' && ch != '}' {
            result.push(ch);
        }
    }

    result
}
