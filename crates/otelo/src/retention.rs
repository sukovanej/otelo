use std::num::NonZeroU16;

use anyhow::ensure;
use otelo_indexed_storage_sqlite::RetentionDays;
use otelo_query::Signal;

const DEFAULT_JOURNAL_RETENTION_DAYS: NonZeroU16 = NonZeroU16::new(30).expect("thirty is not zero");
const DEFAULT_INDEX_RETENTION_DAYS: NonZeroU16 = NonZeroU16::new(7).expect("seven is not zero");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retention {
    journal_days: NonZeroU16,
    index_days: RetentionDays,
}

impl Retention {
    pub fn new(journal_days: NonZeroU16, index_days: RetentionDays) -> anyhow::Result<Self> {
        for signal in Signal::ALL {
            let days = index_days.of_signal(signal);
            ensure!(
                days <= journal_days,
                "the index keeps {signal} for {days} days and the journal for {journal_days}, \
                 so otelo reindex could not rebuild the days the journal no longer has"
            );
        }
        Ok(Self {
            journal_days,
            index_days,
        })
    }

    pub fn from_defaults() -> anyhow::Result<Self> {
        Self::new(
            DEFAULT_JOURNAL_RETENTION_DAYS,
            RetentionDays {
                logs: DEFAULT_INDEX_RETENTION_DAYS,
                traces: DEFAULT_INDEX_RETENTION_DAYS,
                metrics: DEFAULT_INDEX_RETENTION_DAYS,
            },
        )
    }

    #[must_use]
    pub const fn journal_days(&self) -> NonZeroU16 {
        self.journal_days
    }

    #[must_use]
    pub const fn index_days(&self) -> RetentionDays {
        self.index_days
    }
}
