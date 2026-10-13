use crate::error::IngestError;
use crate::types::PageRoute;
use docula_core::{BoundingBox, SpatialSpan, SpreadsheetSubtype};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::io::{Cursor, Read};
use zip::ZipArchive;

/// Extracted tabular cell containing text, grid coordinates, and cell reference.
#[derive(Clone, Debug, PartialEq)]
pub struct SpreadsheetCell {
    /// Excel-style alphanumeric coordinate (e.g., "A1", "C14").
    pub reference: String,
    /// 0-indexed row index.
    pub row: usize,
    /// 0-indexed column index.
    pub col: usize,
    /// Raw textual value.
    pub value: String,
}

/// Extracted worksheet containing cells and grid boundaries.
#[derive(Clone, Debug, PartialEq)]
pub struct SpreadsheetSheet {
    /// Worksheet name or title.
    pub name: String,
    /// 0-indexed sheet order.
    pub index: usize,
    /// Ordered list of populated cells.
    pub cells: Vec<SpreadsheetCell>,
    /// Highest row index encountered.
    pub max_row: usize,
    /// Highest column index encountered.
    pub max_col: usize,
}

impl SpreadsheetSheet {
    /// Creates a new worksheet container.
    #[must_use]
    pub fn new(name: impl Into<String>, index: usize) -> Self {
        Self {
            name: name.into(),
            index,
            cells: Vec::new(),
            max_row: 0,
            max_col: 0,
        }
    }

    /// Adds a cell to the worksheet, tracking maximum dimensions.
    pub fn add_cell(&mut self, cell: SpreadsheetCell) {
        if cell.row > self.max_row {
            self.max_row = cell.row;
        }
        if cell.col > self.max_col {
            self.max_col = cell.col;
        }
        self.cells.push(cell);
    }

    /// Projects tabular cells into normalized $[0.0, 1.0]$ spatial spans.
    #[must_use]
    pub fn to_spatial_spans(&self) -> Vec<SpatialSpan> {
        let total_rows = (self.max_row + 1).max(1) as f32;
        let total_cols = (self.max_col + 1).max(1) as f32;

        let row_height = 1.0 / total_rows;
        let col_width = 1.0 / total_cols;

        let mut spans = Vec::with_capacity(self.cells.len());

        for cell in &self.cells {
            let x0 = (cell.col as f32 * col_width).clamp(0.0, 1.0);
            let y0 = (cell.row as f32 * row_height).clamp(0.0, 1.0);
            let x1 = (x0 + col_width * 0.95).clamp(0.0, 1.0);
            let y1 = (y0 + row_height * 0.90).clamp(0.0, 1.0);

            let bbox = BoundingBox::from_coords_clamped(x0, y0, x1, y1);
            spans.push(SpatialSpan {
                text: cell.value.clone(),
                bbox,
                page_index: self.index,
                confidence: 1.0,
            });
        }

        spans
    }
}

/// Extracted spreadsheet workbook containing multiple sheets.
#[derive(Clone, Debug, PartialEq)]
pub struct SpreadsheetWorkbook {
    /// Sheets in the workbook.
    pub sheets: Vec<SpreadsheetSheet>,
}

impl SpreadsheetWorkbook {
    /// Converts workbook sheets into born-digital page routes.
    #[must_use]
    pub fn to_page_routes(&self) -> Vec<PageRoute> {
        self.sheets
            .iter()
            .enumerate()
            .map(|(idx, sheet)| PageRoute::BornDigital {
                page_index: idx,
                text_layer: sheet.to_spatial_spans(),
            })
            .collect()
    }
}

/// Pure-Rust parser for spreadsheet and tabular file formats.
pub struct SpreadsheetReader;

impl SpreadsheetReader {
    /// Ingests spreadsheet bytes based on subtype into a [`SpreadsheetWorkbook`].
    pub fn read_bytes(
        subtype: SpreadsheetSubtype,
        bytes: &[u8],
    ) -> Result<SpreadsheetWorkbook, IngestError> {
        if bytes.is_empty() {
            return Err(IngestError::EmptyPayload);
        }

        match subtype {
            SpreadsheetSubtype::Csv => Self::read_csv(bytes),
            SpreadsheetSubtype::Tsv => Self::read_tsv(bytes),
            SpreadsheetSubtype::Xlsx => Self::read_xlsx(bytes),
            SpreadsheetSubtype::Ods => Self::read_ods(bytes),
            SpreadsheetSubtype::Xls => Err(IngestError::OfficeFormat(
                "Legacy binary .xls format is deprecated; please convert to .xlsx or .csv".into(),
            )),
        }
    }

    /// Parses RFC 4180 CSV byte streams.
    pub fn read_csv(bytes: &[u8]) -> Result<SpreadsheetWorkbook, IngestError> {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(bytes));

        let mut sheet = SpreadsheetSheet::new("Sheet1", 0);

        for (row_idx, result) in rdr.records().enumerate() {
            let record = result.map_err(|e| IngestError::OfficeFormat(e.to_string()))?;
            for (col_idx, field) in record.iter().enumerate() {
                let trimmed = field.trim();
                if !trimmed.is_empty() {
                    let cell_ref = format_cell_reference(row_idx, col_idx);
                    sheet.add_cell(SpreadsheetCell {
                        reference: cell_ref,
                        row: row_idx,
                        col: col_idx,
                        value: trimmed.to_string(),
                    });
                }
            }
        }

        Ok(SpreadsheetWorkbook {
            sheets: vec![sheet],
        })
    }

    /// Parses tab-separated value (TSV) byte streams.
    pub fn read_tsv(bytes: &[u8]) -> Result<SpreadsheetWorkbook, IngestError> {
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(bytes));

        let mut sheet = SpreadsheetSheet::new("Sheet1", 0);

        for (row_idx, result) in rdr.records().enumerate() {
            let record = result.map_err(|e| IngestError::OfficeFormat(e.to_string()))?;
            for (col_idx, field) in record.iter().enumerate() {
                let trimmed = field.trim();
                if !trimmed.is_empty() {
                    let cell_ref = format_cell_reference(row_idx, col_idx);
                    sheet.add_cell(SpreadsheetCell {
                        reference: cell_ref,
                        row: row_idx,
                        col: col_idx,
                        value: trimmed.to_string(),
                    });
                }
            }
        }

        Ok(SpreadsheetWorkbook {
            sheets: vec![sheet],
        })
    }

    /// Parses Microsoft Excel OpenXML (`.xlsx`) ZIP streams.
    pub fn read_xlsx(bytes: &[u8]) -> Result<SpreadsheetWorkbook, IngestError> {
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| IngestError::OfficeFormat(format!("Failed to open XLSX zip: {e}")))?;

        let shared_strings = extract_shared_strings(&mut archive);

        let mut sheet_names = Vec::new();
        for i in 0..archive.len() {
            let file = archive
                .by_index(i)
                .map_err(|e| IngestError::OfficeFormat(e.to_string()))?;
            let name = file.name().to_string();
            if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
                sheet_names.push(name);
            }
        }
        sheet_names.sort();

        if sheet_names.is_empty() {
            return Err(IngestError::OfficeFormat(
                "No worksheets found in XLSX".into(),
            ));
        }

        let mut sheets = Vec::with_capacity(sheet_names.len());

        for (sheet_idx, name) in sheet_names.into_iter().enumerate() {
            let mut sheet_file = archive
                .by_name(&name)
                .map_err(|e| IngestError::OfficeFormat(e.to_string()))?;
            let mut xml_data = String::new();
            sheet_file
                .read_to_string(&mut xml_data)
                .map_err(|e| IngestError::OfficeFormat(e.to_string()))?;

            let sheet = parse_worksheet_xml(&xml_data, &shared_strings, sheet_idx)?;
            sheets.push(sheet);
        }

        Ok(SpreadsheetWorkbook { sheets })
    }

    /// Parses OpenDocument Spreadsheet (`.ods`) ZIP streams.
    pub fn read_ods(bytes: &[u8]) -> Result<SpreadsheetWorkbook, IngestError> {
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| IngestError::OfficeFormat(format!("Failed to open ODS zip: {e}")))?;

        let mut content_file = archive
            .by_name("content.xml")
            .map_err(|e| IngestError::OfficeFormat(format!("ODS missing content.xml: {e}")))?;

        let mut xml_data = String::new();
        content_file
            .read_to_string(&mut xml_data)
            .map_err(|e| IngestError::OfficeFormat(e.to_string()))?;

        parse_ods_content_xml(&xml_data)
    }
}

/// Resolves shared strings table from `xl/sharedStrings.xml`.
fn extract_shared_strings(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Vec<String> {
    let mut strings = Vec::new();
    if let Ok(mut file) = archive.by_name("xl/sharedStrings.xml") {
        let mut xml = String::new();
        if file.read_to_string(&mut xml).is_ok() {
            let mut reader = Reader::from_str(&xml);
            let mut buf = Vec::new();
            let mut in_t = false;
            let mut current_str = String::new();

            while let Ok(event) = reader.read_event_into(&mut buf) {
                match event {
                    Event::Start(ref e) if e.name().as_ref() == "t" => {
                        in_t = true;
                    }
                    Event::End(ref e) if e.name().as_ref() == "t" => {
                        in_t = false;
                    }
                    Event::End(ref e) if e.name().as_ref() == "si" => {
                        strings.push(std::mem::take(&mut current_str));
                    }
                    Event::Text(ref e) if in_t => {
                        current_str.push_str(e.as_ref());
                    }
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
        }
    }
    strings
}

/// Parses individual XLSX worksheet XML into a [`SpreadsheetSheet`].
fn parse_worksheet_xml(
    xml: &str,
    shared_strings: &[String],
    sheet_idx: usize,
) -> Result<SpreadsheetSheet, IngestError> {
    let mut sheet = SpreadsheetSheet::new(format!("Sheet{}", sheet_idx + 1), sheet_idx);
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut current_ref = String::new();
    let mut current_type = String::new();
    let mut in_value = false;
    let mut cell_value = String::new();

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(ref e) if e.name().as_ref() == "c" => {
                current_ref.clear();
                current_type.clear();
                cell_value.clear();
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == "r" {
                        current_ref = attr.value.to_string();
                    } else if attr.key.as_ref() == "t" {
                        current_type = attr.value.to_string();
                    }
                }
            }
            Event::Start(ref e) if e.name().as_ref() == "v" || e.name().as_ref() == "t" => {
                in_value = true;
            }
            Event::End(ref e) if e.name().as_ref() == "v" || e.name().as_ref() == "t" => {
                in_value = false;
            }
            Event::End(ref e) if e.name().as_ref() == "c" => {
                let trimmed = cell_value.trim();
                if !trimmed.is_empty() && !current_ref.is_empty() {
                    let final_val = if current_type == "s" {
                        trimmed
                            .parse::<usize>()
                            .ok()
                            .and_then(|idx| shared_strings.get(idx))
                            .cloned()
                            .unwrap_or_else(|| trimmed.to_string())
                    } else {
                        trimmed.to_string()
                    };

                    let (r, c) = parse_cell_reference(&current_ref);
                    sheet.add_cell(SpreadsheetCell {
                        reference: current_ref.clone(),
                        row: r,
                        col: c,
                        value: final_val,
                    });
                }
            }
            Event::Text(ref e) if in_value => {
                cell_value.push_str(e.as_ref());
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(sheet)
}

/// Parses ODS `content.xml` stream into a [`SpreadsheetWorkbook`].
fn parse_ods_content_xml(xml: &str) -> Result<SpreadsheetWorkbook, IngestError> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut sheets = Vec::new();
    let mut current_sheet: Option<SpreadsheetSheet> = None;
    let mut current_row: usize = 0;
    let mut current_col: usize = 0;
    let mut in_cell_p = false;
    let mut cell_text = String::new();
    let mut col_repeat: usize = 1;

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(ref e) if e.name().as_ref() == "table:table" => {
                let name = e
                    .attributes()
                    .flatten()
                    .find(|a| a.key.as_ref() == "table:name")
                    .map(|a| a.value.to_string())
                    .unwrap_or_else(|| "Sheet".to_string());
                let idx = sheets.len();
                current_sheet = Some(SpreadsheetSheet::new(name, idx));
                current_row = 0;
            }
            Event::End(ref e) if e.name().as_ref() == "table:table" => {
                if let Some(s) = current_sheet.take() {
                    sheets.push(s);
                }
            }
            Event::Start(ref e) if e.name().as_ref() == "table:table-row" => {
                current_col = 0;
            }
            Event::End(ref e) if e.name().as_ref() == "table:table-row" => {
                current_row += 1;
            }
            Event::Start(ref e) if e.name().as_ref() == "table:table-cell" => {
                cell_text.clear();
                col_repeat = e
                    .attributes()
                    .flatten()
                    .find(|a| a.key.as_ref() == "table:number-columns-repeated")
                    .and_then(|a| a.value.parse::<usize>().ok())
                    .unwrap_or(1);
            }
            Event::End(ref e) if e.name().as_ref() == "table:table-cell" => {
                let trimmed = cell_text.trim();
                if !trimmed.is_empty()
                    && let Some(ref mut sheet) = current_sheet
                {
                    let cell_ref = format_cell_reference(current_row, current_col);
                    sheet.add_cell(SpreadsheetCell {
                        reference: cell_ref,
                        row: current_row,
                        col: current_col,
                        value: trimmed.to_string(),
                    });
                }
                current_col += col_repeat.min(100);
            }
            Event::Start(ref e) if e.name().as_ref() == "text:p" => {
                in_cell_p = true;
            }
            Event::End(ref e) if e.name().as_ref() == "text:p" => {
                in_cell_p = false;
            }
            Event::Text(ref e) if in_cell_p => {
                cell_text.push_str(e.as_ref());
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    if sheets.is_empty() {
        sheets.push(SpreadsheetSheet::new("Sheet1", 0));
    }

    Ok(SpreadsheetWorkbook { sheets })
}

/// Parses standard cell reference (e.g. "B12") into 0-indexed row and column numbers.
fn parse_cell_reference(ref_str: &str) -> (usize, usize) {
    let mut col: usize = 0;
    let mut row_str = String::new();

    for ch in ref_str.chars() {
        if ch.is_ascii_alphabetic() {
            let val = (ch.to_ascii_uppercase() as u8 - b'A' + 1) as usize;
            col = col.saturating_mul(26).saturating_add(val);
        } else if ch.is_ascii_digit() {
            row_str.push(ch);
        }
    }

    let col_idx = col.saturating_sub(1);
    let row_idx = row_str.parse::<usize>().unwrap_or(1).saturating_sub(1);
    (row_idx, col_idx)
}

/// Formats 0-indexed row and column into Excel-style cell coordinates (e.g. `(0, 0) -> "A1"`).
fn format_cell_reference(row: usize, mut col: usize) -> String {
    let mut col_str = String::new();
    loop {
        let rem = (col % 26) as u8;
        col_str.insert(0, (b'A' + rem) as char);
        if col < 26 {
            break;
        }
        col = (col / 26) - 1;
    }
    format!("{}{}", col_str, row + 1)
}
