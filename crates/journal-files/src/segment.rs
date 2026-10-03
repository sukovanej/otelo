use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::Context;
use otelo_journal::Hour;
use otelo_query::Signal;

const UNCOMPRESSED_EXTENSION: &str = ".seg";
const COMPRESSED_EXTENSION: &str = ".seg.zst";
const TEMPORARY_EXTENSION: &str = ".tmp";
const HOUR_FORMAT: &str = "%Y-%m-%dT%H";

pub const fn directory_name_of_signal(signal: Signal) -> &'static str {
    match signal {
        Signal::Logs => "logs",
        Signal::Spans => "traces",
        Signal::Metrics => "metrics",
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SegmentFiles {
    pub uncompressed: bool,
    pub compressed: bool,
}

pub struct SegmentDirectory {
    path: PathBuf,
}

impl SegmentDirectory {
    pub fn new(journal_directory: &Path, signal: Signal) -> Self {
        Self {
            path: journal_directory.join(directory_name_of_signal(signal)),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn uncompressed_segment_path(&self, hour: Hour) -> PathBuf {
        self.path
            .join(format!("{}{UNCOMPRESSED_EXTENSION}", format_hour(hour)))
    }

    pub fn compressed_segment_path(&self, hour: Hour) -> PathBuf {
        self.path
            .join(format!("{}{COMPRESSED_EXTENSION}", format_hour(hour)))
    }

    pub fn temporary_compressed_segment_path(&self, hour: Hour) -> PathBuf {
        self.path.join(format!(
            "{}{COMPRESSED_EXTENSION}{TEMPORARY_EXTENSION}",
            format_hour(hour)
        ))
    }

    pub fn list_segments(&self) -> anyhow::Result<BTreeMap<Hour, SegmentFiles>> {
        let mut segments = BTreeMap::<Hour, SegmentFiles>::new();
        for file_name in self.list_file_names()? {
            if let Some(hour) = file_name
                .strip_suffix(COMPRESSED_EXTENSION)
                .and_then(parse_hour)
            {
                segments.entry(hour).or_default().compressed = true;
            } else if let Some(hour) = file_name
                .strip_suffix(UNCOMPRESSED_EXTENSION)
                .and_then(parse_hour)
            {
                segments.entry(hour).or_default().uncompressed = true;
            }
        }
        Ok(segments)
    }

    pub fn delete_temporary_files(&self) -> anyhow::Result<()> {
        for file_name in self.list_file_names()? {
            if file_name.ends_with(TEMPORARY_EXTENSION) {
                let path = self.path.join(file_name);
                fs::remove_file(&path).with_context(|| format!("delete {}", path.display()))?;
            }
        }
        Ok(())
    }

    pub fn size_in_bytes(&self) -> anyhow::Result<u64> {
        let mut bytes = 0;
        for file_name in self.list_file_names()? {
            let path = self.path.join(file_name);
            match fs::metadata(&path) {
                Ok(metadata) => bytes += metadata.len(),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("read the size of {}", path.display()));
                }
            }
        }
        Ok(bytes)
    }

    pub fn sync(&self) -> anyhow::Result<()> {
        File::open(&self.path)
            .and_then(|directory| directory.sync_all())
            .with_context(|| format!("sync the directory {}", self.path.display()))
    }

    fn list_file_names(&self) -> anyhow::Result<Vec<String>> {
        let entries =
            fs::read_dir(&self.path).with_context(|| format!("list {}", self.path.display()))?;
        let mut file_names = Vec::new();
        for entry in entries {
            let entry = entry.with_context(|| format!("list {}", self.path.display()))?;
            if let Ok(file_name) = entry.file_name().into_string() {
                file_names.push(file_name);
            }
        }
        Ok(file_names)
    }
}

fn format_hour(hour: Hour) -> String {
    jiff::Timestamp::from_second(hour.hours_since_epoch() * 3_600)
        .expect("a journal hour is a valid instant")
        .strftime(HOUR_FORMAT)
        .to_string()
}

fn parse_hour(text: &str) -> Option<Hour> {
    let started_at: jiff::Timestamp = format!("{text}:00:00Z").parse().ok()?;
    Some(Hour::from_hours_since_epoch(
        started_at.as_second().div_euclid(3_600),
    ))
}
