use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Context;
use otelo_indexed_storage::{IndexedAttribute, TimeRange};
use rusqlite::{Connection, OpenFlags};

use crate::telemetry_file::{TELEMETRY_FILE_NAME, keep_small_page_cache};

pub struct Reader {
    connection: Connection,
    range: TimeRange,
    indexed_attributes: BTreeSet<IndexedAttribute>,
}

impl Reader {
    pub fn open(directory: &Path, range: TimeRange) -> anyhow::Result<Self> {
        let _entered = tracing::info_span!("open reader").entered();
        let path = directory.join(TELEMETRY_FILE_NAME);
        // Without SQLITE_OPEN_CREATE, a missing file is an error and not an empty file.
        let connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("open {}", path.display()))?;
        keep_small_page_cache(&connection)?;
        connection.pragma_update(None, "query_only", true)?;
        Ok(Self {
            connection,
            range,
            indexed_attributes: BTreeSet::new(),
        })
    }

    pub fn set_time_limit(&self, limit: Duration) -> anyhow::Result<()> {
        let deadline = Instant::now() + limit;
        self.connection
            .progress_handler(1000, Some(move || Instant::now() > deadline))?;
        Ok(())
    }

    pub fn set_indexed_attributes(&mut self, attributes: BTreeSet<IndexedAttribute>) {
        self.indexed_attributes = attributes;
    }

    #[must_use]
    pub const fn indexed_attributes(&self) -> &BTreeSet<IndexedAttribute> {
        &self.indexed_attributes
    }

    #[must_use]
    pub const fn range(&self) -> TimeRange {
        self.range
    }

    #[must_use]
    pub const fn connection(&self) -> &Connection {
        &self.connection
    }
}

pub fn timed_out(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<rusqlite::Error>()
            .and_then(rusqlite::Error::sqlite_error_code)
            == Some(rusqlite::ErrorCode::OperationInterrupted)
    })
}
