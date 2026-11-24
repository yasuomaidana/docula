//! Builds small in-memory ZIP archives for the office-format tests.
//!
//! Included with `#[path]` by `office_ingest_tests.rs` and the workspace root's
//! `office_pipeline_tests.rs` so both share one writer.

use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;

/// Packs `(entry name, contents)` pairs into an uncompressed ZIP archive.
pub fn zip_archive(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, contents) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(contents.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    buf
}
