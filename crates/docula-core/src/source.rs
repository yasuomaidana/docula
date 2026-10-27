use crate::office::OfficeFormat;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Universal document input abstraction supporting PDFs, raster images, office files, and batches.
#[derive(Clone, Debug)]
pub enum DocumentSource {
    /// In-memory PDF byte stream.
    PdfBytes(Vec<u8>),
    /// Filesystem path pointing to a PDF file.
    PdfPath(PathBuf),
    /// In-memory raster image byte stream with optional MIME or format indicator.
    ImageBytes {
        /// Optional format hint (e.g. "png", "jpeg", "webp").
        format: Option<String>,
        /// Raw image file binary content.
        data: Vec<u8>,
    },
    /// Filesystem path pointing to a raster image file.
    ImagePath(PathBuf),
    /// In-memory office suite document byte stream.
    OfficeBytes {
        /// Detected or declared office document format.
        format: OfficeFormat,
        /// Raw document file binary content.
        data: Vec<u8>,
    },
    /// Filesystem path pointing to an office suite document.
    OfficePath {
        /// Filesystem path.
        path: PathBuf,
        /// Detected or declared office document format.
        format: OfficeFormat,
    },
    /// In-memory batch of raw image byte payloads.
    BatchImages(Vec<Vec<u8>>),
    /// Batch of multiple document files on disk (e.g. multi-page photo set).
    BatchFiles(Vec<PathBuf>),
}

impl DocumentSource {
    /// Constructs a `DocumentSource` from in-memory PDF bytes.
    #[inline]
    pub fn from_pdf_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self::PdfBytes(bytes.into())
    }

    /// Constructs a `DocumentSource` from a PDF file path.
    #[inline]
    pub fn from_pdf_path(path: impl Into<PathBuf>) -> Self {
        Self::PdfPath(path.into())
    }

    /// Constructs a `DocumentSource` from in-memory image bytes.
    #[inline]
    pub fn from_image_bytes(data: impl Into<Vec<u8>>, format: Option<&str>) -> Self {
        Self::ImageBytes {
            format: format.map(str::to_string),
            data: data.into(),
        }
    }

    /// Constructs a `DocumentSource` from an image file path.
    #[inline]
    pub fn from_image_path(path: impl Into<PathBuf>) -> Self {
        Self::ImagePath(path.into())
    }

    /// Constructs a `DocumentSource` from in-memory office document bytes.
    #[inline]
    pub fn from_office_bytes(data: impl Into<Vec<u8>>, format: OfficeFormat) -> Self {
        Self::OfficeBytes {
            format,
            data: data.into(),
        }
    }

    /// Constructs a `DocumentSource` from an office document path with auto-detected or provided format.
    #[inline]
    pub fn from_office_path(
        path: impl Into<PathBuf>,
        format: Option<OfficeFormat>,
    ) -> Option<Self> {
        let path = path.into();
        let detected = format.or_else(|| OfficeFormat::detect(Some(&path), None))?;
        Some(Self::OfficePath {
            path,
            format: detected,
        })
    }

    /// Constructs a `DocumentSource` from a collection of in-memory image byte buffers.
    #[inline]
    pub fn from_batch_images<I, B>(images: I) -> Self
    where
        I: IntoIterator<Item = B>,
        B: Into<Vec<u8>>,
    {
        Self::BatchImages(images.into_iter().map(Into::into).collect())
    }

    /// Constructs a `DocumentSource` from a collection of file paths.
    #[inline]
    pub fn from_batch_files<I, P>(paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        Self::BatchFiles(paths.into_iter().map(Into::into).collect())
    }

    /// Returns a borrowed slice of the underlying bytes if stored in memory.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::PdfBytes(bytes) => Some(bytes.as_slice()),
            Self::ImageBytes { data, .. } | Self::OfficeBytes { data, .. } => Some(data.as_slice()),
            _ => None,
        }
    }

    /// Returns a borrowed reference to the file path if backed by the filesystem.
    pub fn as_path(&self) -> Option<&Path> {
        match self {
            Self::PdfPath(path) | Self::ImagePath(path) | Self::OfficePath { path, .. } => {
                Some(path.as_path())
            }
            _ => None,
        }
    }

    /// Returns `true` if the source represents a PDF document.
    pub fn is_pdf(&self) -> bool {
        match self {
            Self::PdfBytes(_) => true,
            Self::PdfPath(path) => path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf")),
            _ => false,
        }
    }

    /// Returns `true` if the source represents a raster image.
    pub fn is_image(&self) -> bool {
        match self {
            Self::ImageBytes { .. } | Self::BatchImages(_) => true,
            Self::ImagePath(path) => {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy();
                    matches!(
                        ext_str.to_ascii_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "webp" | "tiff" | "tif" | "bmp"
                    )
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Returns `true` if the source represents an ofimatic document.
    pub fn is_office(&self) -> bool {
        matches!(self, Self::OfficeBytes { .. } | Self::OfficePath { .. })
    }

    /// Returns the office format if the source is an office document.
    pub fn office_format(&self) -> Option<OfficeFormat> {
        match self {
            Self::OfficeBytes { format, .. } | Self::OfficePath { format, .. } => Some(*format),
            _ => None,
        }
    }

    /// Returns `true` if the source represents a spreadsheet or tabular document.
    pub fn is_spreadsheet(&self) -> bool {
        self.office_format().is_some_and(|f| f.is_spreadsheet())
    }

    /// Returns `true` if the source represents a presentation slide deck.
    pub fn is_presentation(&self) -> bool {
        self.office_format().is_some_and(|f| f.is_presentation())
    }

    /// Returns `true` if the source represents a word processing or textual document.
    pub fn is_word_processing(&self) -> bool {
        self.office_format().is_some_and(|f| f.is_word_processing())
    }

    /// Returns `true` if the source represents a batch of images or files.
    #[inline]
    pub fn is_batch(&self) -> bool {
        matches!(self, Self::BatchImages(_) | Self::BatchFiles(_))
    }

    /// Returns byte length if the payload is in memory.
    pub fn byte_len(&self) -> Option<usize> {
        self.as_bytes().map(<[u8]>::len)
    }

    /// Automatically infers or derives a stable document identifier for this source.
    ///
    /// # Inference Strategy
    /// - **Filesystem Paths** ([`PdfPath`](Self::PdfPath), [`ImagePath`](Self::ImagePath), [`OfficePath`](Self::OfficePath)):
    ///   Extracts the file stem (e.g. `"invoice_example"` from `"tests/fixtures/invoice_example.png"`).
    /// - **In-Memory Payloads** ([`PdfBytes`](Self::PdfBytes), [`ImageBytes`](Self::ImageBytes), [`OfficeBytes`](Self::OfficeBytes)):
    ///   Derives a deterministic SHA-256 fingerprint hex digest (`doc-<16-hex-chars>`).
    /// - **Batch Files** ([`BatchFiles`](Self::BatchFiles)):
    ///   Extracts the first file stem and appends the batch count (e.g. `"page1-batch-3"`).
    /// - **Batch In-Memory Images** ([`BatchImages`](Self::BatchImages)):
    ///   Hashes all image buffers into a combined digest (`doc-batch-<16-hex-chars>`).
    ///
    /// This enables zero-configuration document parsing workflows where an upstream tracking
    /// identifier is not available, allowing callers to parse documents directly from source.
    pub fn infer_document_id(&self) -> String {
        match self {
            Self::PdfPath(path) | Self::ImagePath(path) | Self::OfficePath { path, .. } => path
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| "doc-unnamed-file".to_string()),
            Self::PdfBytes(bytes) => Self::digest_to_id("doc", bytes),
            Self::ImageBytes { data, .. } | Self::OfficeBytes { data, .. } => {
                Self::digest_to_id("doc", data)
            }
            Self::BatchFiles(paths) => {
                if let Some(first) = paths.first() {
                    let stem = first
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .filter(|s| !s.is_empty())
                        .unwrap_or("file");
                    format!("{stem}-batch-{}", paths.len())
                } else {
                    "doc-batch-empty".to_string()
                }
            }
            Self::BatchImages(images) => {
                let mut hasher = Sha256::new();
                for img in images {
                    hasher.update(img);
                }
                let digest = hasher.finalize();
                let short: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
                format!("doc-batch-{short}")
            }
        }
    }

    #[inline]
    fn digest_to_id(prefix: &str, bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = hasher.finalize();
        let short: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
        format!("{prefix}-{short}")
    }
}

impl From<Vec<u8>> for DocumentSource {
    fn from(bytes: Vec<u8>) -> Self {
        Self::PdfBytes(bytes)
    }
}

impl From<PathBuf> for DocumentSource {
    fn from(path: PathBuf) -> Self {
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
        {
            Self::PdfPath(path)
        } else if let Some(office_fmt) = OfficeFormat::detect(Some(&path), None) {
            Self::OfficePath {
                path,
                format: office_fmt,
            }
        } else {
            Self::ImagePath(path)
        }
    }
}

impl From<&Path> for DocumentSource {
    fn from(path: &Path) -> Self {
        path.to_path_buf().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::office::SpreadsheetSubtype;

    #[test]
    fn test_infer_document_id_from_paths() {
        let pdf_src =
            DocumentSource::from_pdf_path(PathBuf::from("/tmp/invoices/INV-2026-001.pdf"));
        assert_eq!(pdf_src.infer_document_id(), "INV-2026-001");

        let img_src = DocumentSource::from_image_path(PathBuf::from("assets/receipt_dec_12.png"));
        assert_eq!(img_src.infer_document_id(), "receipt_dec_12");

        let office_src = DocumentSource::from_office_path(
            PathBuf::from("reports/q3_earnings.xlsx"),
            Some(OfficeFormat::Spreadsheet(SpreadsheetSubtype::Xlsx)),
        )
        .expect("valid office source");
        assert_eq!(office_src.infer_document_id(), "q3_earnings");
    }

    #[test]
    fn test_infer_document_id_from_bytes_deterministic() {
        let payload = b"Hello Docula Sovereign Invoice Intelligence";
        let pdf_src = DocumentSource::from_pdf_bytes(payload.to_vec());
        let img_src = DocumentSource::from_image_bytes(payload.to_vec(), Some("png"));

        let pdf_id = pdf_src.infer_document_id();
        let img_id = img_src.infer_document_id();

        assert!(pdf_id.starts_with("doc-"));
        assert_eq!(
            pdf_id, img_id,
            "Identical payloads must yield identical IDs"
        );
        assert_eq!(pdf_id.len(), 20, "Prefix 'doc-' (4) + 16 hex chars = 20");
    }

    #[test]
    fn test_infer_document_id_batches() {
        let empty_batch = DocumentSource::from_batch_files(Vec::<PathBuf>::new());
        assert_eq!(empty_batch.infer_document_id(), "doc-batch-empty");

        let file_batch = DocumentSource::from_batch_files(vec![
            PathBuf::from("page_001.png"),
            PathBuf::from("page_002.png"),
        ]);
        assert_eq!(file_batch.infer_document_id(), "page_001-batch-2");

        let img_batch = DocumentSource::from_batch_images(vec![vec![1, 2, 3], vec![4, 5, 6]]);
        assert!(img_batch.infer_document_id().starts_with("doc-batch-"));
    }

    #[test]
    fn test_infer_document_id_unnamed_fallback() {
        let unnamed = DocumentSource::from_pdf_path(PathBuf::from("/"));
        assert_eq!(unnamed.infer_document_id(), "doc-unnamed-file");
    }
}
