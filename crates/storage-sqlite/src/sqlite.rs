use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use otelo_storage::{
    BatchInbox, IndexedAttribute, MetricRetention, RangeQueries, Result, Storage, StorageSize,
    TimeRange,
};

use crate::day::Day;
use crate::indexes::Indexes;
use crate::rollup::ROLLUP_FILE_NAME;
use crate::state::StateFile;
use crate::{Config, Reader, Writer};

pub struct Sqlite {
    config: Config,
    state: StateFile,
}

impl Sqlite {
    pub fn open(data_directory: &Path) -> anyhow::Result<Self> {
        let state = StateFile::open(data_directory)?;
        let mut config = Config::new(data_directory.join("telemetry"));
        config.indexes = Indexes::new(state.indexed_attributes()?);
        Ok(Self { config, state })
    }

    pub fn spawn_writer(&self, inbox: BatchInbox) -> anyhow::Result<Writer> {
        Writer::spawn(self.config.clone(), inbox)
    }
}

impl Storage for Sqlite {
    fn oldest_retained_at(&self) -> i64 {
        self.config.oldest_retained_day(Day::today()).start_at()
    }

    fn metric_retention(&self) -> MetricRetention {
        self.config.metric_retention(Day::today())
    }

    fn size(&self) -> Result<StorageSize> {
        let is_rollup_file_name = |name: &str| name.starts_with(ROLLUP_FILE_NAME);
        Ok(StorageSize {
            telemetry_bytes: size_of_files_in_bytes(&self.config.directory, |name| {
                !is_rollup_file_name(name)
            })?,
            rollup_bytes: size_of_files_in_bytes(&self.config.directory, is_rollup_file_name)?,
            state_bytes: self.state.size_in_bytes()?,
        })
    }

    fn open_range(&self, range: TimeRange, time_limit: Duration) -> Result<Box<dyn RangeQueries>> {
        let mut reader = Reader::open(&self.config.directory, range)?;
        reader.set_time_limit(time_limit)?;
        reader.set_indexed_attributes(self.config.indexes.attributes());
        Ok(Box::new(reader))
    }

    fn indexed_attributes(&self) -> BTreeSet<IndexedAttribute> {
        self.config.indexes.attributes()
    }

    fn add_index(&self, attribute: &IndexedAttribute) -> Result<()> {
        self.state.add_indexed_attribute(attribute)?;
        self.config
            .indexes
            .replace_attributes(self.state.indexed_attributes()?);
        Ok(())
    }

    fn remove_index(&self, attribute: &IndexedAttribute) -> Result<bool> {
        let removed = self.state.remove_indexed_attribute(attribute)?;
        self.config
            .indexes
            .replace_attributes(self.state.indexed_attributes()?);
        Ok(removed)
    }
}

// Counts every file whose name passes, so a write-ahead log is part of the size.
fn size_of_files_in_bytes(
    directory: &Path,
    is_counted_name: impl Fn(&str) -> bool,
) -> anyhow::Result<u64> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        // The writer makes the directory when it starts.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error).with_context(|| format!("list {}", directory.display())),
    };
    let mut total_bytes = 0;
    for entry in entries {
        let entry = entry.with_context(|| format!("list {}", directory.display()))?;
        if !entry.file_name().to_str().is_some_and(&is_counted_name) {
            continue;
        }
        match entry.metadata() {
            Ok(metadata) if metadata.is_file() => total_bytes += metadata.len(),
            Ok(_) => {}
            // Retention deleted the file between the listing and this read.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("read the size of {}", entry.path().display()));
            }
        }
    }
    Ok(total_bytes)
}
