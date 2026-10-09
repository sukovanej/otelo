use std::num::NonZeroU16;

use otelo::retention::Retention;
use otelo_indexed_storage_sqlite::RetentionDays;

const fn days(count: u16) -> NonZeroU16 {
    NonZeroU16::new(count).expect("a retention is at least a day")
}

#[test]
fn the_default_retention_keeps_the_journal_longer_than_the_index() {
    Retention::from_defaults().unwrap();
}

#[test]
fn an_index_may_keep_a_signal_as_long_as_the_journal() {
    let index_days = RetentionDays {
        logs: days(30),
        traces: days(7),
        metrics: days(7),
    };
    let retention = Retention::new(days(30), index_days).unwrap();
    assert_eq!(retention.index_days(), index_days);
}

#[test]
fn an_index_that_keeps_a_signal_longer_than_the_journal_is_refused() {
    let index_days = RetentionDays {
        logs: days(7),
        traces: days(31),
        metrics: days(7),
    };
    let error = Retention::new(days(30), index_days).unwrap_err();
    assert_eq!(
        error.to_string(),
        "the index keeps spans for 31 days and the journal for 30, so otelo reindex could not \
         rebuild the days the journal no longer has"
    );
}
