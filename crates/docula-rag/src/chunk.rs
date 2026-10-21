use docula_core::BoundingBox;
use docula_ocr::{SpatialLayoutMap, TextLine};
use serde::{Deserialize, Serialize};

/// Cohesive semantic text chunk preserving spatial bounding boxes and table structures.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocumentChunk {
    /// Zero-based chunk identifier within the document.
    pub id: usize,
    /// Origin page number (zero-indexed).
    pub page_index: usize,
    /// Textual content formatted with layout headers.
    pub text: String,
    /// Bounding box enclosing all content in this chunk.
    pub bbox: BoundingBox,
    /// Flag indicating whether this chunk contains a structured table grid.
    pub is_table: bool,
    /// Estimated token count (roughly whitespace-split words * 1.3).
    pub token_estimate: usize,
}

/// Chunker that keeps tabular grids intact while chunking paragraphs up to token limits.
#[derive(Clone, Debug)]
pub struct TableAwareChunker {
    /// Maximum estimated tokens per chunk.
    pub max_chunk_tokens: usize,
}

impl Default for TableAwareChunker {
    fn default() -> Self {
        Self {
            max_chunk_tokens: 500,
        }
    }
}

impl TableAwareChunker {
    /// Creates a new `TableAwareChunker` with a target maximum tokens per chunk.
    #[must_use]
    pub const fn new(max_chunk_tokens: usize) -> Self {
        Self { max_chunk_tokens }
    }

    /// Chunks a `SpatialLayoutMap` into semantic chunks, guaranteeing tables remain intact.
    #[must_use]
    pub fn chunk_layout(
        &self,
        page_index: usize,
        layout: &SpatialLayoutMap,
        start_id: usize,
    ) -> Vec<DocumentChunk> {
        if layout.is_empty() {
            return Vec::new();
        }

        let mut chunks = Vec::new();
        let mut current_id = start_id;

        let mut current_lines: Vec<&TextLine> = Vec::new();
        let mut current_is_table = false;
        let mut current_tokens = 0;

        for line in &layout.lines {
            let line_is_table = is_table_line(line);
            let line_tokens = estimate_tokens(&line.text);

            let mode_changed = !current_lines.is_empty() && (line_is_table != current_is_table);
            let limit_exceeded =
                !current_is_table && (current_tokens + line_tokens > self.max_chunk_tokens);

            if mode_changed || limit_exceeded {
                if let Some(chunk) =
                    create_chunk(current_id, page_index, &current_lines, current_is_table)
                {
                    chunks.push(chunk);
                    current_id += 1;
                }
                current_lines.clear();
                current_tokens = 0;
            }

            current_is_table = line_is_table;
            current_lines.push(line);
            current_tokens += line_tokens;
        }

        if let Some(chunk) = create_chunk(current_id, page_index, &current_lines, current_is_table)
        {
            chunks.push(chunk);
        }

        chunks
    }
}

fn is_table_line(line: &TextLine) -> bool {
    line.spans.len() >= 2 || line.text.contains("    ")
}

fn estimate_tokens(text: &str) -> usize {
    let word_count = text.split_whitespace().count();
    (word_count * 4).div_ceil(3)
}

fn create_chunk(
    id: usize,
    page_index: usize,
    lines: &[&TextLine],
    is_table: bool,
) -> Option<DocumentChunk> {
    if lines.is_empty() {
        return None;
    }

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    let mut text_buf = String::new();
    if is_table {
        text_buf.push_str(&format!("[PAGE {}: Table Grid]\n", page_index + 1));
    } else {
        text_buf.push_str(&format!("[PAGE {}: Section]\n", page_index + 1));
    }

    for line in lines {
        min_x = min_x.min(line.bbox.top_left.x);
        min_y = min_y.min(line.bbox.top_left.y);
        max_x = max_x.max(line.bbox.bottom_right.x);
        max_y = max_y.max(line.bbox.bottom_right.y);

        text_buf.push_str(&line.text);
        text_buf.push('\n');
    }

    let bbox = BoundingBox::from_coords_clamped(min_x, min_y, max_x, max_y);
    let token_estimate = estimate_tokens(&text_buf);

    Some(DocumentChunk {
        id,
        page_index,
        text: text_buf,
        bbox,
        is_table,
        token_estimate,
    })
}
