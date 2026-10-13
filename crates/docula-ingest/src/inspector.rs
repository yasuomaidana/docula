use crate::error::IngestError;
use crate::types::RouterConfig;
use docula_core::{BoundingBox, SpatialSpan};
use lopdf::content::Content;
use lopdf::{Document, Object, ObjectId};

/// Internal evaluation details for a single PDF page.
#[derive(Debug, Clone)]
pub struct PageInspection {
    /// 0-indexed page index.
    pub page_index: usize,
    /// 1-indexed PDF page number.
    pub page_number: u32,
    /// Whether valid, decodable digital text meeting length thresholds is present.
    pub has_valid_text: bool,
    /// Whether any font has a missing, corrupt, or unparsable `/ToUnicode` stream.
    pub is_tounicode_corrupted: bool,
    /// Whether the page contains embedded raster image XObjects.
    pub has_images: bool,
    /// Extracted spatial text spans for born-digital routing.
    pub spatial_spans: Vec<SpatialSpan>,
}

/// Low-level PDF object tree inspector built on `lopdf`.
pub struct PdfInspector;

impl PdfInspector {
    /// Inspects an individual page within the given `lopdf::Document`.
    ///
    /// # Errors
    /// Returns [`IngestError::PdfStructure`] if the page dictionary cannot be resolved.
    pub fn inspect_page(
        doc: &Document,
        page_index: usize,
        page_number: u32,
        page_id: ObjectId,
        config: &RouterConfig,
    ) -> Result<PageInspection, IngestError> {
        let page_dict = doc
            .get_object(page_id)
            .and_then(Object::as_dict)
            .map_err(|e| {
                IngestError::PdfStructure(format!("Page {page_number} dict not found: {e}"))
            })?;

        // 1. Inspect Fonts and /ToUnicode mapping streams
        let is_tounicode_corrupted = Self::evaluate_page_fonts(doc, page_dict, page_id);

        // 2. Inspect embedded XObject Images
        let has_images = Self::inspect_embedded_images(doc, page_dict);

        // 3. Extract digital text and check text quality
        let (has_valid_text, extracted_text) =
            Self::evaluate_page_text(doc, page_number, config, is_tounicode_corrupted);

        // 4. Construct spatial text spans if born-digital
        let mut spatial_spans = Vec::new();
        if has_valid_text && !is_tounicode_corrupted {
            spatial_spans =
                Self::extract_spatial_spans(doc, page_dict, page_id, page_index, &extracted_text);
        }

        Ok(PageInspection {
            page_index,
            page_number,
            has_valid_text,
            is_tounicode_corrupted,
            has_images,
            spatial_spans,
        })
    }

    /// Evaluates all fonts on the page to verify `/ToUnicode` stream integrity.
    fn evaluate_page_fonts(
        doc: &Document,
        _page_dict: &lopdf::Dictionary,
        page_id: ObjectId,
    ) -> bool {
        let fonts = match doc.get_page_fonts(page_id) {
            Ok(f) => f,
            Err(_) => return false,
        };

        for (_name, font_dict) in fonts {
            // Check /ToUnicode stream presence and validity
            if let Ok(to_unicode_obj) = font_dict.get(b"ToUnicode") {
                if Self::is_to_unicode_object_corrupted(doc, to_unicode_obj) {
                    return true;
                }
            } else if let Ok(subtype) = font_dict.get(b"Subtype").and_then(Object::as_name)
                && subtype == b"Type0"
                && let Ok(encoding) = font_dict.get(b"Encoding").and_then(Object::as_name)
                && (encoding == b"Identity-H" || encoding == b"Identity-V")
            {
                // If font is a Type0 (CID) font with Identity encoding and lacks ToUnicode,
                // glyph IDs cannot be translated to Unicode characters!
                return true;
            }
        }

        false
    }

    /// Checks if a `/ToUnicode` object is invalid, un-decompressible, empty, or lacks CMap syntax.
    fn is_to_unicode_object_corrupted(doc: &Document, obj: &Object) -> bool {
        let resolved = match obj {
            Object::Reference(id) => match doc.get_object(*id) {
                Ok(o) => o,
                Err(_) => return true,
            },
            other => other,
        };

        match resolved {
            Object::Stream(stream) => {
                let content = match stream.decompressed_content() {
                    Ok(c) => c,
                    Err(_) => return true,
                };

                if content.is_empty() {
                    return true;
                }

                // Check for standard CMap tokens
                let has_cmap_tokens = content
                    .windows(9)
                    .any(|w| w == b"begincmap" || w == b"beginCMap")
                    || content.windows(11).any(|w| w == b"beginbfchar")
                    || content.windows(12).any(|w| w == b"beginbfrange")
                    || content.windows(7).any(|w| w == b"CIDInit");

                !has_cmap_tokens
            }
            _ => true,
        }
    }

    /// Inspects whether the page references any `/XObject` with subtype `/Image`.
    fn inspect_embedded_images(doc: &Document, page_dict: &lopdf::Dictionary) -> bool {
        let resources = match page_dict.get(b"Resources") {
            Ok(r) => match doc.dereference(r) {
                Ok((_, obj)) => obj.as_dict().ok(),
                Err(_) => None,
            },
            Err(_) => None,
        };

        let resources_dict = match resources {
            Some(d) => d,
            None => return false,
        };

        let xobjects = match resources_dict.get(b"XObject") {
            Ok(xo) => match doc.dereference(xo) {
                Ok((_, obj)) => obj.as_dict().ok(),
                Err(_) => None,
            },
            Err(_) => None,
        };

        let xobject_dict = match xobjects {
            Some(d) => d,
            None => return false,
        };

        for (_key, val) in xobject_dict.iter() {
            if let Ok((_, obj)) = doc.dereference(val)
                && let Ok(dict) = obj.as_dict()
                && let Ok(subtype) = dict.get(b"Subtype").and_then(Object::as_name)
                && subtype == b"Image"
            {
                return true;
            }
        }

        false
    }

    /// Extracts text and evaluates character counts and replacement character ratios.
    fn evaluate_page_text(
        doc: &Document,
        page_number: u32,
        config: &RouterConfig,
        is_tounicode_corrupted: bool,
    ) -> (bool, String) {
        if is_tounicode_corrupted {
            return (false, String::new());
        }

        let text = match doc.extract_text(&[page_number]) {
            Ok(t) => t,
            Err(_) => return (false, String::new()),
        };

        let non_whitespace: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
        let non_whitespace_count = non_whitespace.len();

        if non_whitespace_count < config.min_text_chars {
            return (false, text);
        }

        // Count replacement chars (\u{FFFD} or unmapped control characters)
        let replacement_count = non_whitespace
            .iter()
            .filter(|&&c| c == '\u{FFFD}' || c == '\0')
            .count();

        let ratio = replacement_count as f32 / non_whitespace_count as f32;
        if ratio > config.max_replacement_char_ratio {
            return (false, text);
        }

        (true, text)
    }

    /// Constructs spatial spans from the page's text streams or lines.
    fn extract_spatial_spans(
        doc: &Document,
        page_dict: &lopdf::Dictionary,
        page_id: ObjectId,
        page_index: usize,
        extracted_text: &str,
    ) -> Vec<SpatialSpan> {
        let mut spans = Vec::new();

        // Retrieve page boundary (MediaBox or CropBox)
        let (page_w, page_h) = Self::get_page_dimensions(doc, page_dict);

        // Attempt fine-grained content stream operation parsing
        let content_data = doc.get_page_content(page_id);
        if !content_data.is_empty()
            && let Ok(content) = Content::decode(&content_data)
        {
            spans = Self::parse_content_operations(&content, page_index, page_w, page_h);
        }

        // If content stream parsing yielded no spans, fallback to line-level layout
        if spans.is_empty() {
            let lines: Vec<&str> = extracted_text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect();

            let total_lines = lines.len().max(1);
            let line_height = 0.85 / (total_lines as f32);

            for (idx, line) in lines.into_iter().enumerate() {
                let top_y = (0.05 + idx as f32 * line_height).clamp(0.0, 0.95);
                let bottom_y = (top_y + line_height.min(0.04)).clamp(top_y + 0.01, 1.0);
                let bbox = BoundingBox::from_coords_clamped(0.05, top_y, 0.95, bottom_y);

                if let Ok(span) = SpatialSpan::new(line.to_string(), bbox, page_index, 1.0) {
                    spans.push(span);
                }
            }
        }

        spans
    }

    /// Extracts `(width, height)` in points from the page dictionary.
    fn get_page_dimensions(doc: &Document, page_dict: &lopdf::Dictionary) -> (f32, f32) {
        let box_entry = page_dict
            .get(b"CropBox")
            .or_else(|_| page_dict.get(b"MediaBox"));
        if let Ok(entry) = box_entry
            && let Ok((_, obj)) = doc.dereference(entry)
            && let Ok(arr) = obj.as_array()
            && arr.len() == 4
        {
            let x0 = arr[0]
                .as_float()
                .or_else(|_| arr[0].as_i64().map(|i| i as f32))
                .unwrap_or(0.0);
            let y0 = arr[1]
                .as_float()
                .or_else(|_| arr[1].as_i64().map(|i| i as f32))
                .unwrap_or(0.0);
            let x1 = arr[2]
                .as_float()
                .or_else(|_| arr[2].as_i64().map(|i| i as f32))
                .unwrap_or(612.0);
            let y1 = arr[3]
                .as_float()
                .or_else(|_| arr[3].as_i64().map(|i| i as f32))
                .unwrap_or(792.0);
            return ((x1 - x0).abs().max(1.0), (y1 - y0).abs().max(1.0));
        }
        (612.0, 792.0) // Standard US Letter default
    }

    /// Parses text matrices (`Tm`, `Td`, `T*`) and show operators to extract positioned spans.
    fn parse_content_operations(
        content: &Content,
        page_index: usize,
        page_w: f32,
        page_h: f32,
    ) -> Vec<SpatialSpan> {
        let mut spans = Vec::new();
        let mut curr_x = 0.0f32;
        let mut curr_y = 0.0f32;
        let mut font_size = 12.0f32;

        for op in &content.operations {
            match op.operator.as_str() {
                "Tf" => {
                    if let Some(size_obj) = op.operands.get(1) {
                        font_size = size_obj
                            .as_float()
                            .or_else(|_| size_obj.as_i64().map(|i| i as f32))
                            .unwrap_or(12.0)
                            .max(1.0);
                    }
                }
                "Tm" => {
                    if op.operands.len() >= 6 {
                        curr_x = op.operands[4]
                            .as_float()
                            .or_else(|_| op.operands[4].as_i64().map(|i| i as f32))
                            .unwrap_or(0.0);
                        curr_y = op.operands[5]
                            .as_float()
                            .or_else(|_| op.operands[5].as_i64().map(|i| i as f32))
                            .unwrap_or(0.0);
                    }
                }
                "Td" | "TD" => {
                    if op.operands.len() >= 2 {
                        let tx = op.operands[0]
                            .as_float()
                            .or_else(|_| op.operands[0].as_i64().map(|i| i as f32))
                            .unwrap_or(0.0);
                        let ty = op.operands[1]
                            .as_float()
                            .or_else(|_| op.operands[1].as_i64().map(|i| i as f32))
                            .unwrap_or(0.0);
                        curr_x += tx;
                        curr_y += ty;
                    }
                }
                "T*" => {
                    curr_y -= font_size * 1.2;
                }
                "Tj" | "'" | "\"" => {
                    if let Some(text_obj) = op.operands.first() {
                        let text_str = Self::extract_string_from_object(text_obj);
                        if !text_str.trim().is_empty() {
                            let norm_x0 = (curr_x / page_w).clamp(0.0, 1.0);
                            let norm_y0 = (1.0 - (curr_y + font_size) / page_h).clamp(0.0, 1.0);
                            let char_w = font_size * 0.5 * text_str.len() as f32;
                            let norm_x1 = ((curr_x + char_w) / page_w).clamp(norm_x0 + 0.01, 1.0);
                            let norm_y1 =
                                (norm_y0 + (font_size * 1.2) / page_h).clamp(norm_y0 + 0.01, 1.0);

                            let bbox = BoundingBox::from_coords_clamped(
                                norm_x0, norm_y0, norm_x1, norm_y1,
                            );
                            if let Ok(span) = SpatialSpan::new(text_str, bbox, page_index, 1.0) {
                                spans.push(span);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        spans
    }

    /// Extracts string from an Object (String or Array).
    fn extract_string_from_object(obj: &Object) -> String {
        match obj {
            Object::String(bytes, _) => String::from_utf8_lossy(bytes).into_owned(),
            _ => String::new(),
        }
    }
}
