use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, ensure};
use otelo_storage::{IndexedAttribute, TimeRange};
use rusqlite::Connection;

use crate::day::Day;
use crate::rollup::ROLLUP_FILE_NAME;
use crate::writer::create_schema;

// SQLite attaches 10 files at most, and the rollups are one of them.
const MAX_ATTACHED_DAYS: usize = 9;

const TABLES: [&str; 7] = [
    "resources",
    "logs",
    "spans",
    "series",
    "points",
    "attribute_keys",
    "attribute_values",
];

pub struct Reader {
    conn: Connection,
    days: Vec<Day>,
    has_rollups: bool,
    range: TimeRange,
    indexed_attributes: BTreeSet<IndexedAttribute>,
}

impl Reader {
    pub fn open(dir: &Path, range: TimeRange) -> anyhow::Result<Self> {
        let span = tracing::info_span!("open reader", days = tracing::field::Empty);
        let _entered = span.enter();
        let conn = Connection::open_in_memory()?;
        // The empty main tables give the views their columns when no day file is attached.
        create_schema(&conn)?;
        let mut days = Vec::new();
        let mut day = Day::of(range.start_at());
        while day <= Day::of(range.end_at() - 1) {
            if dir.join(day.file_name()).is_file() {
                days.push(day);
            }
            day = day.plus(1);
        }
        ensure!(
            days.len() <= MAX_ATTACHED_DAYS,
            "the range covers {} day files, and a query reads at most {MAX_ATTACHED_DAYS}",
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
        let rollups = dir.join(ROLLUP_FILE_NAME);
        let has_rollups = rollups.is_file();
        if has_rollups {
            conn.execute("ATTACH DATABASE ?1 AS rollup", [rollups.to_string_lossy()])
                .with_context(|| format!("attach {}", rollups.display()))?;
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
            has_rollups,
            range,
            indexed_attributes: BTreeSet::new(),
        })
    }

    pub fn set_time_limit(&self, limit: Duration) -> anyhow::Result<()> {
        let deadline = Instant::now() + limit;
        self.conn
            .progress_handler(1000, Some(move || Instant::now() > deadline))?;
        Ok(())
    }

    #[must_use]
    pub const fn has_rollups(&self) -> bool {
        self.has_rollups
    }

    #[must_use]
    pub fn days(&self) -> &[Day] {
        &self.days
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
    pub const fn conn(&self) -> &Connection {
        &self.conn
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
