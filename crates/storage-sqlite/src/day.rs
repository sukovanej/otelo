use std::fmt;
use std::str::FromStr;

use jiff::civil::{Date, date};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};

const NANOS_PER_DAY: i64 = 86_400 * 1_000_000_000;
const UNIX_EPOCH_DATE: Date = date(1970, 1, 1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Day(i64);

impl Day {
    #[must_use]
    pub const fn from_unix_nanos(unix_nanos: i64) -> Self {
        Self(unix_nanos.div_euclid(NANOS_PER_DAY))
    }

    #[must_use]
    pub fn today() -> Self {
        Self::from_unix_nanos(otelo_storage::now_unix_nanos())
    }

    #[must_use]
    pub const fn start_at(self) -> i64 {
        self.0 * NANOS_PER_DAY
    }

    #[must_use]
    pub const fn add_days(self, days: i64) -> Self {
        Self(self.0 + days)
    }

    // A file per day lets retention delete whole files.
    #[must_use]
    pub fn file_name(self) -> String {
        format!("{self}.sqlite")
    }

    #[must_use]
    pub fn from_file_name(name: &str) -> Option<Self> {
        let stem = name
            .strip_suffix(".sqlite")
            .or_else(|| name.strip_suffix(".sqlite-wal"))
            .or_else(|| name.strip_suffix(".sqlite-shm"))
            .or_else(|| stem_of_a_file_set_aside(name))?;
        stem.parse().ok()
    }

    fn date(self) -> Date {
        UNIX_EPOCH_DATE
            .checked_add(jiff::Span::new().days(self.0))
            .expect("a day of an i64 nanosecond timestamp is a valid date")
    }
}

impl fmt::Display for Day {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.date().fmt(formatter)
    }
}

impl FromStr for Day {
    type Err = jiff::Error;

    fn from_str(date: &str) -> Result<Self, Self::Err> {
        let date: Date = date.parse()?;
        Ok(Self(i64::from(UNIX_EPOCH_DATE.until(date)?.get_days())))
    }
}

impl FromSql for Day {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value
            .as_str()?
            .parse()
            .map_err(|error: jiff::Error| FromSqlError::Other(Box::new(error)))
    }
}

// The writer sets a day file of another schema version aside under this name, and retention
// deletes it with its day.
fn stem_of_a_file_set_aside(name: &str) -> Option<&str> {
    let (stem, version) = name.split_once(".sqlite.schema-")?;
    version.parse::<i32>().ok().map(|_| stem)
}
