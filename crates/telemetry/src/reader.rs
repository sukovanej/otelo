use std::fmt::Write;
use std::path::Path;

use anyhow::{Context, ensure};
use rusqlite::Connection;

use crate::day::Day;
use crate::writer::SCHEMA;

/// The most day files one reader attaches: `SQLITE_MAX_ATTACHED` in the
/// bundled SQLite.
const MAX_DAYS: usize = 10;

const TABLES: [&str; 5] = ["resources", "logs", "spans", "series", "points"];

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
        Ok(Self { conn, days })
    }

    /// The days whose files are attached, oldest first.
    #[must_use]
    pub fn days(&self) -> &[Day] {
        &self.days
    }

    #[must_use]
    pub const fn conn(&self) -> &Connection {
        &self.conn
    }
}
