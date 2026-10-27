use crate::error::IngestError;
use crate::types::PageRoute;
use docula_core::{BoundingBox, PresentationSubtype, SpatialSpan};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::io::{Cursor, Read};
use zip::ZipArchive;

/// Extracted presentation slide with ordered text elements and spatial positions.
#[derive(Clone, Debug, PartialEq)]
pub struct PresentationSlide {
    /// 0-indexed slide position.
    pub slide_index: usize,
    /// Extracted text elements with spatial bounds.
    pub spans: Vec<SpatialSpan>,
}

impl PresentationSlide {
    /// Converts slide into born-digital page route.
    #[must_use]
    pub fn to_page_route(&self) -> PageRoute {
        PageRoute::BornDigital {
            page_index: self.slide_index,
            text_layer: self.spans.clone(),
        }
    }
}

/// Extracted presentation deck containing slides.
#[derive(Clone, Debug, PartialEq)]
pub struct PresentationDeck {
    /// Ordered slides in the presentation.
    pub slides: Vec<PresentationSlide>,
}

impl PresentationDeck {
    /// Converts deck slides into born-digital page routes.
    #[must_use]
    pub fn to_page_routes(&self) -> Vec<PageRoute> {
        self.slides
            .iter()
            .map(PresentationSlide::to_page_route)
            .collect()
    }
}

/// Pure-Rust parser for slide presentation formats (.pptx, .odp).
pub struct PresentationReader;

impl PresentationReader {
    /// Ingests presentation bytes based on subtype into a [`PresentationDeck`].
    pub fn read_bytes(
        subtype: PresentationSubtype,
        bytes: &[u8],
    ) -> Result<PresentationDeck, IngestError> {
        if bytes.is_empty() {
            return Err(IngestError::EmptyPayload);
        }

        match subtype {
            PresentationSubtype::Pptx => Self::read_pptx(bytes),
            PresentationSubtype::Odp => Self::read_odp(bytes),
            PresentationSubtype::Ppt => Err(IngestError::OfficeFormat(
                "Legacy binary .ppt format is deprecated; please convert to .pptx".into(),
            )),
        }
    }

    /// Parses Microsoft PowerPoint OpenXML (`.pptx`) ZIP streams.
    pub fn read_pptx(bytes: &[u8]) -> Result<PresentationDeck, IngestError> {
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| IngestError::OfficeFormat(format!("Failed to open PPTX zip: {e}")))?;

        let mut slide_names = Vec::new();
        for i in 0..archive.len() {
            let file = archive
                .by_index(i)
                .map_err(|e| IngestError::OfficeFormat(e.to_string()))?;
            let name = file.name().to_string();
            if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                slide_names.push(name);
            }
        }
        slide_names.sort_by_key(|n| extract_slide_number(n));

        if slide_names.is_empty() {
            return Err(IngestError::OfficeFormat(
                "No slides found in PPTX presentation".into(),
            ));
        }

        let mut slides = Vec::with_capacity(slide_names.len());

        for (slide_idx, name) in slide_names.into_iter().enumerate() {
            let mut slide_file = archive
                .by_name(&name)
                .map_err(|e| IngestError::OfficeFormat(e.to_string()))?;
            let mut xml_data = String::new();
            slide_file
                .read_to_string(&mut xml_data)
                .map_err(|e| IngestError::OfficeFormat(e.to_string()))?;

            let slide = parse_pptx_slide_xml(&xml_data, slide_idx)?;
            slides.push(slide);
        }

        Ok(PresentationDeck { slides })
    }

    /// Parses OpenDocument Presentation (`.odp`) ZIP streams.
    pub fn read_odp(bytes: &[u8]) -> Result<PresentationDeck, IngestError> {
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| IngestError::OfficeFormat(format!("Failed to open ODP zip: {e}")))?;

        let mut content_file = archive
            .by_name("content.xml")
            .map_err(|e| IngestError::OfficeFormat(format!("ODP missing content.xml: {e}")))?;

        let mut xml_data = String::new();
        content_file
            .read_to_string(&mut xml_data)
            .map_err(|e| IngestError::OfficeFormat(e.to_string()))?;

        parse_odp_content_xml(&xml_data)
    }
}

#[inline]
fn commit_paragraph(current: &mut String, acc: &mut Vec<String>) {
    let text = std::mem::take(current);
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        acc.push(trimmed.to_string());
    }
}

/// Parses individual PPTX slide XML into a [`PresentationSlide`].
fn parse_pptx_slide_xml(xml: &str, slide_idx: usize) -> Result<PresentationSlide, IngestError> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut paragraphs: Vec<String> = Vec::new();
    let mut current_para = String::new();
    let mut in_text = false;

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(ref e) if e.name().as_ref() == "a:t" => {
                in_text = true;
            }
            Event::End(ref e) if e.name().as_ref() == "a:t" => {
                in_text = false;
            }
            Event::End(ref e) if e.name().as_ref() == "a:p" => {
                commit_paragraph(&mut current_para, &mut paragraphs);
            }
            Event::Text(ref e) if in_text => {
                current_para.push_str(e.as_ref());
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let total = paragraphs.len().max(1) as f32;
    let step = 0.85 / total;
    let mut spans = Vec::with_capacity(paragraphs.len());

    for (i, p) in paragraphs.into_iter().enumerate() {
        let y0 = (0.08 + i as f32 * step).clamp(0.0, 1.0);
        let y1 = (y0 + step * 0.80).clamp(0.0, 1.0);

        let bbox = BoundingBox::from_coords_clamped(0.08, y0, 0.92, y1);
        spans.push(SpatialSpan {
            text: p,
            bbox,
            page_index: slide_idx,
            confidence: 1.0,
        });
    }

    Ok(PresentationSlide {
        slide_index: slide_idx,
        spans,
    })
}

/// Parses ODP `content.xml` stream into a [`PresentationDeck`].
fn parse_odp_content_xml(xml: &str) -> Result<PresentationDeck, IngestError> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut slides = Vec::new();
    let mut current_paras: Vec<String> = Vec::new();
    let mut in_p = false;
    let mut current_text = String::new();
    let mut slide_idx = 0;

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(ref e) if e.name().as_ref() == "draw:page" => {
                current_paras.clear();
            }
            Event::End(ref e) if e.name().as_ref() == "draw:page" => {
                let total = current_paras.len().max(1) as f32;
                let step = 0.85 / total;
                let mut spans = Vec::new();

                for (i, p) in current_paras.drain(..).enumerate() {
                    let y0 = (0.08 + i as f32 * step).clamp(0.0, 1.0);
                    let y1 = (y0 + step * 0.80).clamp(0.0, 1.0);

                    let bbox = BoundingBox::from_coords_clamped(0.08, y0, 0.92, y1);
                    spans.push(SpatialSpan {
                        text: p,
                        bbox,
                        page_index: slide_idx,
                        confidence: 1.0,
                    });
                }

                slides.push(PresentationSlide {
                    slide_index: slide_idx,
                    spans,
                });
                slide_idx += 1;
            }
            Event::Start(ref e) if e.name().as_ref() == "text:p" => {
                in_p = true;
                current_text.clear();
            }
            Event::End(ref e) if e.name().as_ref() == "text:p" => {
                in_p = false;
                let trimmed = current_text.trim();
                if !trimmed.is_empty() {
                    current_paras.push(trimmed.to_string());
                }
            }
            Event::Text(ref e) if in_p => {
                current_text.push_str(e.as_ref());
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    if slides.is_empty() {
        slides.push(PresentationSlide {
            slide_index: 0,
            spans: Vec::new(),
        });
    }

    Ok(PresentationDeck { slides })
}

/// Helper extracting integer index from slide file name (e.g. "ppt/slides/slide3.xml" -> 3).
fn extract_slide_number(name: &str) -> usize {
    name.strip_prefix("ppt/slides/slide")
        .and_then(|s| s.strip_suffix(".xml"))
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0)
}
