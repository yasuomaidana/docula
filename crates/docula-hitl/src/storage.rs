use crate::error::HitlError;
use crate::triad::CorrectionTriad;
use std::fs::{OpenOptions, create_dir_all};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Append-only cold archive store persisting immutable `CorrectionTriad` records as JSONL.
#[derive(Clone, Debug)]
pub struct ColdArchiveStore {
    base_dir: PathBuf,
}

impl ColdArchiveStore {
    /// Creates a new `ColdArchiveStore` targeting a directory.
    pub fn new(base_dir: impl Into<PathBuf>) -> Result<Self, HitlError> {
        let base_dir = base_dir.into();
        create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    /// Base directory path of the cold archive.
    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Appends a `CorrectionTriad` record to the archive.
    pub fn append(&self, triad: &CorrectionTriad) -> Result<(), HitlError> {
        let date_str = triad.metadata.captured_at.format("%Y-%m-%d").to_string();
        let file_path = self.base_dir.join(format!("{date_str}_triads.jsonl"));

        let serialized = serde_json::to_string(triad)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;

        writeln!(file, "{serialized}")?;
        file.flush()?;
        Ok(())
    }

    /// Reads all triads recorded across all JSONL files in the archive directory.
    pub fn read_all(&self) -> Result<Vec<CorrectionTriad>, HitlError> {
        let mut triads = Vec::new();

        if !self.base_dir.exists() {
            return Ok(triads);
        }

        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                let file = std::fs::File::open(&path)?;
                let reader = BufReader::new(file);

                for line in reader.lines() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    let triad: CorrectionTriad = serde_json::from_str(&line)?;
                    triads.push(triad);
                }
            }
        }

        Ok(triads)
    }

    /// Counts how many triads share the specified layout fingerprint.
    pub fn count_by_fingerprint(&self, fingerprint: u64) -> Result<usize, HitlError> {
        let all = self.read_all()?;
        Ok(all
            .into_iter()
            .filter(|t| t.metadata.layout_fingerprint == fingerprint)
            .count())
    }
}
