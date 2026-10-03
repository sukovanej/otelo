use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use otelo_indexed_storage::{
    BatchInbox, IndexedAttribute, RangeQueries, Result, Storage, TimeRange,
};
use otelo_query::Signal;

use crate::day::Day;
use crate::indexes::Indexes;
use crate::telemetry_file::{TELEMETRY_FILE_NAME, TelemetryFile};
use crate::{Config, Reader, Writer};

// The daemon has about 50 MB, and SQLite fails an allocation past this rather than grow.
const SQLITE_HEAP_LIMIT_BYTES: i64 = 16 * 1024 * 1024;

pub struct Sqlite {
    config: Config,
}

impl Sqlite {
    pub fn open(
        data_directory: &Path,
        indexed_attributes: BTreeSet<IndexedAttribute>,
    ) -> anyhow::Result<Self> {
        // SAFETY: the call only sets a limit, and SQLite reads it under its own mutex.
        unsafe {
            rusqlite::ffi::sqlite3_hard_heap_limit64(SQLITE_HEAP_LIMIT_BYTES);
        }
        let mut config = Config::new(data_directory.join("telemetry"));
        config.indexes = Indexes::new(indexed_attributes);
        // A reader needs the file, and opens it before the writer has written to it.
        TelemetryFile::open(&config.directory)?;
        Ok(Self { config })
    }

    pub fn spawn_writer(&self, inbox: BatchInbox) -> anyhow::Result<Writer> {
        Writer::spawn(self.config.clone(), inbox)
    }
}

impl Storage for Sqlite {
    fn oldest_retained_at(&self, signal: Signal) -> i64 {
        self.config
            .oldest_retained_days(Day::today())
            .of_signal(signal)
            .start_at()
    }

    fn size_in_bytes(&self) -> Result<u64> {
        Ok(size_of_database_in_bytes(
            &self.config.directory.join(TELEMETRY_FILE_NAME),
        )?)
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

    fn replace_indexed_attributes(&self, attributes: BTreeSet<IndexedAttribute>) {
        self.config.indexes.replace_attributes(attributes);
    }
}

// The write-ahead log and its index are part of the database.
fn size_of_database_in_bytes(path: &Path) -> anyhow::Result<u64> {
    ["", "-wal", "-shm"]
        .into_iter()
        .map(|suffix| {
            let mut path = path.to_owned().into_os_string();
            path.push(suffix);
            match std::fs::metadata(&path) {
                Ok(metadata) => Ok(metadata.len()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
                Err(error) => {
                    Err(error).with_context(|| format!("read the size of {}", path.display()))
                }
            }
        })
        .sum()
}
