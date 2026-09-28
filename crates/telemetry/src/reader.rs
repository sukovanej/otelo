use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, ensure};
use rusqlite::Connection;

use crate::day::Day;
use crate::indexes::IndexedKey;
use crate::writer::SCHEMA;

/// The most day files one reader attaches: `SQLITE_MAX_ATTACHED` in the
/// bundled SQLite.
const MAX_DAYS: usize = 10;

const TABLES: [&str; 7] = [
    "resources",
    "logs",
    "spans",
    "series",
    "points",
    "attribute_keys",
    "attribute_values",
];

/// A read-only connection to the day files that a time range covers.
///
/// Each day file is attached under its date, so `"2026-09-27".logs` is the
/// logs of that day. A temporary view of each table (`resources`, `logs`,
/// `spans`, `series`, `points`) joins the days, with a `day` column in front.
/// Ids count from 1 in each file, so a join on `resource_id` or `series_id`
/// also has to match `day`.
pub struct Reader {
    conn: Connection,
    days: Vec<Day>,
    since: i64,
    until: i64,
    indexes: BTreeSet<IndexedKey>,
}

impl Reader {
    /// Attaches the day files from `since` up to `until`, in unix nanoseconds.
    /// A day without a file is left out.
    ///
    /// # Errors
    ///
    /// When a file cannot be attached, or when the range covers more than 10
    /// day files.
    pub fn open(dir: &Path, since: i64, until: i64) -> anyhow::Result<Self> {
        let span = tracing::info_span!("open reader", days = tracing::field::Empty);
        let _entered = span.enter();
        let conn = Connection::open_in_memory()?;
        // The empty tables in the in-memory database give the views their
        // columns when no day file is attached.
        conn.execute_batch(SCHEMA)?;
        let mut days = Vec::new();
        if since < until {
            let mut day = Day::of(since);
            while day <= Day::of(until - 1) {
                if dir.join(day.file_name()).is_file() {
                    days.push(day);
                }
                day = day.plus(1);
            }
        }
        ensure!(
            days.len() <= MAX_DAYS,
            "the range covers {} day files, and a query reads at most {MAX_DAYS}",
            days.len()
        );
        span.record("days", i64::try_from(days.len())?);
        for day in &days {
            let path = dir.join(day.file_name());
            conn.execute(
                "ATTACH DATABASE ?1 AS ?2",
                (path.to_string_lossy(), day.to_string()),
            )
            .with_context(|| format!("attach {}", path.display()))?;
        }
        for table in TABLES {
            let mut view =
                format!("CREATE TEMP VIEW {table} AS SELECT NULL AS day, * FROM main.{table}");
            for day in &days {
                write!(view, " UNION ALL SELECT '{day}', * FROM \"{day}\".{table}")?;
            }
            conn.execute_batch(&view)?;
        }
        conn.pragma_update(None, "query_only", true)?;
        Ok(Self {
            conn,
            days,
            since,
            until,
            indexes: BTreeSet::new(),
        })
    }

    /// Stops every later statement once `limit` has passed since this call.
    /// A stopped statement fails with an error that [`timed_out`] tells apart.
    ///
    /// # Errors
    ///
    /// When the handler cannot be set.
    pub fn set_time_limit(&self, limit: Duration) -> anyhow::Result<()> {
        let deadline = Instant::now() + limit;
        self.conn
            .progress_handler(1000, Some(move || Instant::now() > deadline))?;
        Ok(())
    }

    /// The days whose files are attached, oldest first.
    #[must_use]
    pub fn days(&self) -> &[Day] {
        &self.days
    }

    /// Tells the queries which attributes have an index, so they can report
    /// the ones that do not.
    pub fn set_indexes(&mut self, indexes: BTreeSet<IndexedKey>) {
        self.indexes = indexes;
    }

    #[must_use]
    pub const fn indexes(&self) -> &BTreeSet<IndexedKey> {
        &self.indexes
    }

    /// The start of the range, in unix nanoseconds.
    #[must_use]
    pub const fn since(&self) -> i64 {
        self.since
    }

    /// The end of the range, in unix nanoseconds, not included.
    #[must_use]
    pub const fn until(&self) -> i64 {
        self.until
    }

    #[must_use]
    pub const fn conn(&self) -> &Connection {
        &self.conn
    }
}

/// Whether `error` comes from a statement that ran past the time limit.
#[must_use]
pub fn timed_out(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<rusqlite::Error>()
            .and_then(rusqlite::Error::sqlite_error_code)
            == Some(rusqlite::ErrorCode::OperationInterrupted)
    })
}
