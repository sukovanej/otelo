use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use otelo_indexed_storage::{
    IndexedAttribute, Indexing, PipelineMeters, RangeQueries, Result, Storage, TimeRange,
};
use otelo_journal::{Hour, Journal, SyncedEndInbox};
use otelo_query::Signal;
use tokio::sync::watch;

use crate::completion_cache::CompletionCache;
use crate::day::Day;
use crate::indexes::Indexes;
use crate::lock::TelemetryLock;
use crate::telemetry_file::{TELEMETRY_FILE_NAME, TelemetryFile};
use crate::version::OtherStorageVersion;
use crate::{Config, FrameMapper, Indexer, Reader, index_journal_until_caught_up};

// The daemon has about 50 MB, and SQLite fails an allocation past this rather than grow.
const SQLITE_HEAP_LIMIT_BYTES: i64 = 16 * 1024 * 1024;

pub const TELEMETRY_DIRECTORY_NAME: &str = "telemetry";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReindexProgress {
    DeletedOtherStorageVersion { found_version: i64 },
    IndexedHour { signal: Signal, hour: Hour },
}

pub struct Sqlite {
    config: Config,
    completion_cache: Arc<CompletionCache>,
    indexing: watch::Sender<Indexing>,
    _telemetry_lock: TelemetryLock,
}

impl Sqlite {
    pub fn open(
        data_directory: &Path,
        indexed_attributes: BTreeSet<IndexedAttribute>,
    ) -> anyhow::Result<Self> {
        limit_sqlite_heap();
        let telemetry_lock = TelemetryLock::acquire(data_directory)?;
        let config = build_config(data_directory, indexed_attributes);
        if let Some(other_version) = delete_telemetry_of_other_storage_version(&config.directory)? {
            tracing::warn!("{other_version}, so the indexer builds it again from the journal");
        }
        // A reader needs the file, and opens it before the indexer has written to it.
        TelemetryFile::open(&config.directory)?;
        Ok(Self {
            config,
            completion_cache: Arc::default(),
            indexing: watch::Sender::new(Indexing::STARTING),
            _telemetry_lock: telemetry_lock,
        })
    }

    pub fn spawn_indexer(
        &self,
        journal: Arc<dyn Journal>,
        synced_ends: SyncedEndInbox,
        map_frame: FrameMapper,
        meters: Arc<PipelineMeters>,
    ) -> anyhow::Result<Indexer> {
        Indexer::spawn(
            self.config.clone(),
            journal,
            synced_ends,
            map_frame,
            meters,
            self.indexing.clone(),
        )
    }
}

pub fn reindex_from_journal(
    telemetry_lock: &TelemetryLock,
    indexed_attributes: BTreeSet<IndexedAttribute>,
    journal: Arc<dyn Journal>,
    map_frame: FrameMapper,
    mut report_progress: impl FnMut(ReindexProgress),
) -> anyhow::Result<()> {
    limit_sqlite_heap();
    let config = build_config(telemetry_lock.data_directory(), indexed_attributes);
    if let Some(other_version) = delete_telemetry_of_other_storage_version(&config.directory)? {
        report_progress(ReindexProgress::DeletedOtherStorageVersion {
            found_version: other_version.found_version,
        });
    }
    index_journal_until_caught_up(
        config,
        journal,
        map_frame,
        Arc::new(PipelineMeters::default()),
        |signal, hour| report_progress(ReindexProgress::IndexedHour { signal, hour }),
    )
}

fn delete_telemetry_of_other_storage_version(
    directory: &Path,
) -> anyhow::Result<Option<OtherStorageVersion>> {
    let other_version = match TelemetryFile::open(directory) {
        Ok(_) => return Ok(None),
        Err(error) => error.downcast::<OtherStorageVersion>()?,
    };
    delete_database(&directory.join(TELEMETRY_FILE_NAME))?;
    Ok(Some(other_version))
}

fn limit_sqlite_heap() {
    // SAFETY: the call only sets a limit, and SQLite reads it under its own mutex.
    unsafe {
        rusqlite::ffi::sqlite3_hard_heap_limit64(SQLITE_HEAP_LIMIT_BYTES);
    }
}

fn build_config(data_directory: &Path, indexed_attributes: BTreeSet<IndexedAttribute>) -> Config {
    let mut config = Config::new(data_directory.join(TELEMETRY_DIRECTORY_NAME));
    config.indexes = Indexes::new(indexed_attributes);
    config
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
        reader.share_completion_cache(Arc::clone(&self.completion_cache));
        Ok(Box::new(reader))
    }

    fn indexed_attributes(&self) -> BTreeSet<IndexedAttribute> {
        self.config.indexes.attributes()
    }

    fn replace_indexed_attributes(&self, attributes: BTreeSet<IndexedAttribute>) {
        self.config.indexes.replace_attributes(attributes);
    }

    fn watch_indexing(&self) -> watch::Receiver<Indexing> {
        self.indexing.subscribe()
    }
}

// The write-ahead log and its index are part of the database.
fn list_paths_of_database(path: &Path) -> impl Iterator<Item = PathBuf> {
    ["", "-wal", "-shm"].into_iter().map(|suffix| {
        let mut path = path.to_owned().into_os_string();
        path.push(suffix);
        PathBuf::from(path)
    })
}

fn delete_database(path: &Path) -> anyhow::Result<()> {
    for path in list_paths_of_database(path) {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).with_context(|| format!("delete {}", path.display())),
        }
    }
    Ok(())
}

fn size_of_database_in_bytes(path: &Path) -> anyhow::Result<u64> {
    list_paths_of_database(path)
        .map(|path| match std::fs::metadata(&path) {
            Ok(metadata) => Ok(metadata.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(error) => {
                Err(error).with_context(|| format!("read the size of {}", path.display()))
            }
        })
        .sum()
}
