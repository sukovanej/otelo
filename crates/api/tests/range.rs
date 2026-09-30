use std::sync::Arc;

use jiff::Timestamp;
use otelo_api::{Api, DefaultSince, convert_to_unix_nanos, parse_duration, parse_time};
use otelo_storage_sqlite::{Day, Sqlite};

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
fn caps_the_range_at_the_retention() {
    let dir = tempfile::tempdir().unwrap();
    let api = Api {
        storage: Arc::new(Sqlite::open(dir.path()).unwrap()),
    };
    let range = api
        .resolve_range(Some("30d"), None, DefaultSince::HourBeforeNow)
        .unwrap();
    assert_eq!(range.start_at(), Day::today().add_days(-6).start_at());
    assert!(
        api.resolve_range(Some("1h"), Some("2h"), DefaultSince::HourBeforeNow)
            .is_err()
    );
    assert!(
        api.resolve_range(Some("30d"), Some("20d"), DefaultSince::HourBeforeNow)
            .is_err()
    );
    // The summaries of the metrics are kept longer.
    let range = api.resolve_metric_range(Some("30d"), None).unwrap();
    assert!(range.start_at() < Day::today().add_days(-29).start_at());
    let range = api.resolve_metric_range(Some("200d"), None).unwrap();
    assert_eq!(range.start_at(), Day::today().add_days(-89).start_at());
}
