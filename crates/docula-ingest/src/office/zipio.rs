//! Shared ZIP helpers for the OpenXML and OpenDocument readers.

use crate::error::IngestError;
use std::io::{Cursor, Read};
use zip::ZipArchive;

/// A ZIP archive borrowed from an in-memory office document.
pub(crate) type OfficeArchive<'a> = ZipArchive<Cursor<&'a [u8]>>;

fn office_error(error: impl ToString) -> IngestError {
    IngestError::OfficeFormat(error.to_string())
}

/// Opens `bytes` as a ZIP archive, naming `format` (for example `DOCX`) in the error.
pub(crate) fn open_archive<'a>(
    bytes: &'a [u8],
    format: &str,
) -> Result<OfficeArchive<'a>, IngestError> {
    ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| IngestError::OfficeFormat(format!("Failed to open {format} zip: {e}")))
}

/// Reads the archive entry `name` to a string, naming `format` when it is missing.
pub(crate) fn read_entry(
    archive: &mut OfficeArchive<'_>,
    name: &str,
    format: &str,
) -> Result<String, IngestError> {
    let mut file = archive
        .by_name(name)
        .map_err(|e| IngestError::OfficeFormat(format!("{format} missing {name}: {e}")))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(office_error)?;
    Ok(contents)
}

/// Opens `bytes` and reads the single entry `name` (the `content.xml` of OpenDocument files).
pub(crate) fn read_single_entry(
    bytes: &[u8],
    format: &str,
    name: &str,
) -> Result<String, IngestError> {
    read_entry(&mut open_archive(bytes, format)?, name, format)
}

/// Lists archive entries whose name starts with `prefix` and ends with `.xml`.
pub(crate) fn xml_entry_names(
    archive: &mut OfficeArchive<'_>,
    prefix: &str,
) -> Result<Vec<String>, IngestError> {
    let mut names = Vec::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(office_error)?;
        let name = String::from_utf8_lossy(file.name_raw()).into_owned();
        if name.starts_with(prefix) && name.ends_with(".xml") {
            names.push(name);
        }
    }
    Ok(names)
}
