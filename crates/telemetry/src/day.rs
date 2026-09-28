use std::fmt;

use jiff::civil::{Date, date};

const NANOS_PER_DAY: i64 = 86_400 * 1_000_000_000;
const EPOCH: Date = date(1970, 1, 1);

/// A UTC day. Each day has its own file, `YYYY-MM-DD.sqlite`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Day(i64);

impl Day {
    /// The day that holds `ts`.
    #[must_use]
    pub const fn of(ts: i64) -> Self {
        Self(ts.div_euclid(NANOS_PER_DAY))
    }

    #[must_use]
    pub fn today() -> Self {
        Self::of(now())
    }

    /// Midnight at the start of the day.
    #[must_use]
    pub const fn start(self) -> i64 {
        self.0 * NANOS_PER_DAY
    }

    #[must_use]
    pub const fn plus(self, days: i64) -> Self {
        Self(self.0 + days)
    }

    #[must_use]
    pub fn file_name(self) -> String {
        format!("{self}.sqlite")
    }

    /// The day of a day file, or of its `-wal` or `-shm` file.
    #[must_use]
    pub fn from_file_name(name: &str) -> Option<Self> {
        let stem = name
            .strip_suffix(".sqlite")
            .or_else(|| name.strip_suffix(".sqlite-wal"))
            .or_else(|| name.strip_suffix(".sqlite-shm"))?;
        let date: Date = stem.parse().ok()?;
        Some(Self(i64::from(EPOCH.until(date).ok()?.get_days())))
    }

    fn date(self) -> Date {
        EPOCH
            .checked_add(jiff::Span::new().days(self.0))
            .expect("a day of an i64 nanosecond timestamp is a valid date")
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.date().fmt(f)
    }
}

/// The current time in unix nanoseconds.
///
/// # Panics
///
/// After the year 2262, when the nanoseconds overflow an `i64`.
#[must_use]
pub fn now() -> i64 {
    i64::try_from(jiff::Timestamp::now().as_nanosecond()).expect("now fits an i64 until 2262")
}
