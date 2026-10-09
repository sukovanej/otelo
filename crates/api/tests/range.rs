use std::collections::BTreeSet;
use std::num::NonZeroU16;
use std::sync::Arc;

use jiff::Timestamp;
use otelo_api::{
    Api, DefaultSince, RangeSignals, convert_to_unix_nanos, parse_duration, parse_time,
};
use otelo_indexed_storage_sqlite::{Day, RetentionDays, Sqlite};
use otelo_query::Signal;
use otelo_state::StateFile;
use tokio_util::sync::CancellationToken;

const INDEX_RETENTION_DAYS: RetentionDays = RetentionDays {
    logs: SEVEN_DAYS,
    traces: SEVEN_DAYS,
    metrics: SEVEN_DAYS,
};
const SEVEN_DAYS: NonZeroU16 = NonZeroU16::new(7).expect("seven is not zero");

const HOUR_NS: i64 = 3600 * 1_000_000_000;

#[test]
fn parses_durations_and_timestamps() {
    let now = convert_to_unix_nanos(Timestamp::now());
    assert_eq!(parse_time("1h", now).unwrap(), now - HOUR_NS);
    assert_eq!(parse_time("2d", now).unwrap(), now - 48 * HOUR_NS);
    assert_eq!(parse_duration("500ms").unwrap(), 500_000_000);
    assert_eq!(
        parse_time("2026-09-28T00:00:00Z", now).unwrap(),
        convert_to_unix_nanos("2026-09-28T00:00:00Z".parse().unwrap())
    );
    assert!(parse_time("yesterday", now).is_err());
}

#[test]
fn caps_the_range_at_the_retention_of_its_signals() {
    let dir = tempfile::tempdir().unwrap();
    let api = Api::new(
        Arc::new(Sqlite::open(dir.path(), INDEX_RETENTION_DAYS, BTreeSet::new()).unwrap()),
        Arc::new(StateFile::open(dir.path()).unwrap()),
        CancellationToken::new(),
    );
    let range = api
        .resolve_range_within_retention(
            RangeSignals::One(Signal::Logs),
            Some("30d"),
            None,
            DefaultSince::HourBeforeNow,
        )
        .unwrap();
    assert_eq!(range.start_at(), Day::today().add_days(-6).start_at());
    let range = api
        .resolve_range_within_retention(
            RangeSignals::One(Signal::Metrics),
            None,
            None,
            DefaultSince::OldestRetained,
        )
        .unwrap();
    assert_eq!(range.start_at(), Day::today().add_days(-6).start_at());
    assert!(
        api.resolve_range_within_retention(
            RangeSignals::One(Signal::Spans),
            Some("1h"),
            Some("2h"),
            DefaultSince::HourBeforeNow
        )
        .is_err()
    );
    assert!(
        api.resolve_range_within_retention(
            RangeSignals::SpansAndLogs,
            Some("30d"),
            Some("20d"),
            DefaultSince::HourBeforeNow
        )
        .is_err()
    );
}
