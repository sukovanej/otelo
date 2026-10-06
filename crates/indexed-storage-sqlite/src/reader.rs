use std::cell::Cell;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use otelo_indexed_storage::{IndexedAttribute, TimeRange};
use rusqlite::{Connection, OpenFlags};

use crate::completion_cache::CompletionCache;
use crate::telemetry_file::{TELEMETRY_FILE_NAME, keep_small_page_cache};

// A context that matches few records makes SQLite scan the whole range, whatever the sample size.
const COMPLETION_TIME_BUDGET: Duration = Duration::from_millis(200);

pub struct Reader {
    connection: Connection,
    range: TimeRange,
    indexed_attributes: BTreeSet<IndexedAttribute>,
    times_out_at: Cell<Option<Instant>>,
    completion_time_budget: Duration,
    completion_cache: Arc<CompletionCache>,
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
            times_out_at: Cell::new(None),
            completion_time_budget: COMPLETION_TIME_BUDGET,
            completion_cache: Arc::default(),
        })
    }

    pub fn set_time_limit(&self, limit: Duration) -> anyhow::Result<()> {
        let times_out_at = Instant::now() + limit;
        self.times_out_at.set(Some(times_out_at));
        self.interrupt_at(Some(times_out_at))
    }

    pub const fn set_completion_time_budget(&mut self, budget: Duration) {
        self.completion_time_budget = budget;
    }

    pub(crate) fn run_within_completion_time_budget<T>(
        &self,
        run: impl FnOnce() -> T,
    ) -> anyhow::Result<T> {
        let budget_ends_at = Instant::now() + self.completion_time_budget;
        let interrupts_at = self
            .times_out_at
            .get()
            .map_or(budget_ends_at, |times_out_at| {
                times_out_at.min(budget_ends_at)
            });
        self.interrupt_at(Some(interrupts_at))?;
        let result = run();
        self.interrupt_at(self.times_out_at.get())?;
        Ok(result)
    }

    fn interrupt_at(&self, interrupts_at: Option<Instant>) -> anyhow::Result<()> {
        match interrupts_at {
            Some(interrupts_at) => self
                .connection
                .progress_handler(1000, Some(move || Instant::now() > interrupts_at))?,
            None => self
                .connection
                .progress_handler(1000, None::<fn() -> bool>)?,
        }
        Ok(())
    }

    pub fn share_completion_cache(&mut self, completion_cache: Arc<CompletionCache>) {
        self.completion_cache = completion_cache;
    }

    pub(crate) fn completion_cache(&self) -> &CompletionCache {
        &self.completion_cache
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
