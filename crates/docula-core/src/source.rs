use crate::office::OfficeFormat;
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
