use crate::error::IngestError;
use crate::inspector::PdfInspector;
use crate::office::OfficeInspector;
use crate::types::{DocumentRoutingReport, PageRoute, RasterJob, RouterConfig};
use docula_core::DocumentSource;
use lopdf::Document;
use std::fs;

/// Intelligent document routing orchestrator.
///
/// Evaluates incoming document payloads, inspects PDF font mappings and object hierarchies,
/// parses structured office suite documents (Excel, PowerPoint, Word, CSV, ODS, ODP, ODT),
/// and partitions pages into born-digital vector fast-path or computer vision OCR routes.
#[derive(Clone, Debug, Default)]
pub struct DocumentRouter {
    config: RouterConfig,
}

impl DocumentRouter {
    /// Creates a new `DocumentRouter` with default heuristic configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RouterConfig::default(),
        }
    }

    /// Creates a router with custom heuristic configuration.
    #[must_use]
    pub const fn with_config(config: RouterConfig) -> Self {
        Self { config }
    }

    /// Returns a reference to the router configuration.
    #[must_use]
    pub const fn config(&self) -> &RouterConfig {
        &self.config
    }

    /// Routes an abstract [`DocumentSource`] into an evaluated [`DocumentRoutingReport`].
    ///
    /// # Errors
    /// Returns [`IngestError`] if reading files fails or the payload is invalid/empty.
    pub fn route_source(
        &self,
        source: &DocumentSource,
    ) -> Result<DocumentRoutingReport, IngestError> {
        match source {
            DocumentSource::PdfBytes(bytes) => self.route_pdf_bytes(bytes),
            DocumentSource::PdfPath(path) => {
                let bytes = fs::read(path)?;
                self.route_pdf_bytes(&bytes)
            }
            DocumentSource::OfficeBytes { .. } | DocumentSource::OfficePath { .. } => {
                OfficeInspector::inspect_source(source)
            }
            DocumentSource::ImageBytes { data, .. } => {
                if data.is_empty() {
                    return Err(IngestError::EmptyPayload);
                }
                Ok(DocumentRoutingReport::new(vec![PageRoute::NeedsOcr {
                    page_index: 0,
                    raster_job: RasterJob::new(0, self.config.default_dpi),
                }]))
            }
            DocumentSource::ImagePath(path) => {
                let metadata = fs::metadata(path)?;
                if metadata.len() == 0 {
                    return Err(IngestError::EmptyPayload);
                }
                Ok(DocumentRoutingReport::new(vec![PageRoute::NeedsOcr {
                    page_index: 0,
                    raster_job: RasterJob::new(0, self.config.default_dpi),
                }]))
            }
            DocumentSource::BatchImages(batch) => {
                if batch.is_empty() {
                    return Err(IngestError::EmptyPayload);
                }
                let routes = (0..batch.len())
                    .map(|idx| PageRoute::NeedsOcr {
                        page_index: idx,
                        raster_job: RasterJob::new(idx, self.config.default_dpi),
                    })
                    .collect();
                Ok(DocumentRoutingReport::new(routes))
            }
            DocumentSource::BatchFiles(paths) => {
                if paths.is_empty() {
                    return Err(IngestError::EmptyPayload);
                }
                let routes = (0..paths.len())
                    .map(|idx| PageRoute::NeedsOcr {
                        page_index: idx,
                        raster_job: RasterJob::new(idx, self.config.default_dpi),
                    })
                    .collect();
                Ok(DocumentRoutingReport::new(routes))
            }
        }
    }

    /// Evaluates raw PDF bytes, traversing all pages and classifying routing dispositions.
    ///
    /// # Errors
    /// Returns [`IngestError::EmptyPayload`] if bytes are empty.
    /// Returns [`IngestError::ParseError`] if PDF binary format is invalid.
    pub fn route_pdf_bytes(&self, bytes: &[u8]) -> Result<DocumentRoutingReport, IngestError> {
        if bytes.is_empty() {
            return Err(IngestError::EmptyPayload);
        }

        let doc = Document::load_mem(bytes).map_err(|e| {
            IngestError::ParseError(format!("Failed to load PDF memory stream: {e}"))
        })?;

        let pages = doc.get_pages();
        if pages.is_empty() {
            return Err(IngestError::ParseError(
                "PDF contains zero pages".to_string(),
            ));
        }

        let mut routes = Vec::with_capacity(pages.len());

        for (page_idx, (&page_num, &page_id)) in pages.iter().enumerate() {
            let inspection =
                PdfInspector::inspect_page(&doc, page_idx, page_num, page_id, &self.config)?;

            let route = if inspection.is_tounicode_corrupted {
                PageRoute::NeedsOcr {
                    page_index: page_idx,
                    raster_job: RasterJob::new(page_idx, self.config.default_dpi),
                }
            } else if inspection.has_valid_text {
                PageRoute::BornDigital {
                    page_index: page_idx,
                    text_layer: inspection.spatial_spans,
                }
            } else {
                PageRoute::NeedsOcr {
                    page_index: page_idx,
                    raster_job: RasterJob::new(page_idx, self.config.default_dpi),
                }
            };

            routes.push(route);
        }

        Ok(DocumentRoutingReport::new(routes))
    }
}
