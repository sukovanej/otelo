use std::fmt;

use jiff::civil::{Date, date};

const NANOS_PER_DAY: i64 = 86_400 * 1_000_000_000;
const EPOCH: Date = date(1970, 1, 1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Day(i64);

impl Day {
    #[must_use]
    pub const fn of(ts: i64) -> Self {
        Self(ts.div_euclid(NANOS_PER_DAY))
    }

    #[must_use]
    pub fn today() -> Self {
        Self::of(siner_storage::now_unix_nanos())
    }

    #[must_use]
    pub const fn start(self) -> i64 {
        self.0 * NANOS_PER_DAY
    }

    #[must_use]
    pub const fn plus(self, days: i64) -> Self {
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
