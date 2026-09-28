use std::path::PathBuf;
use std::sync::Arc;

use jiff::Timestamp;
use siner::api::{Api, nanos, parse_duration, parse_time};
use siner::state::State;
use siner_telemetry::{Day, Indexes};

const HOUR: i64 = 3600 * 1_000_000_000;

#[test]
fn parses_durations_and_timestamps() {
    let now = nanos(Timestamp::now());
    assert_eq!(parse_time("1h", now).unwrap(), now - HOUR);
    assert_eq!(parse_time("2d", now).unwrap(), now - 48 * HOUR);
    assert_eq!(parse_duration("500ms").unwrap(), 500_000_000);
    assert_eq!(
        parse_time("2026-09-28T00:00:00Z", now).unwrap(),
        nanos("2026-09-28T00:00:00Z".parse().unwrap())
    );
    assert!(parse_time("yesterday", now).is_err());
}

#[test]
fn caps_the_range_at_the_retention() {
    let dir = tempfile::tempdir().unwrap();
    let api = Api {
        dir: PathBuf::new(),
        retention_days: 7,
        indexes: Indexes::default(),
        state: Arc::new(State::open(dir.path()).unwrap()),
    };
    let (since, _) = api.range(Some("30d"), None, None).unwrap();
    assert_eq!(since, Day::today().plus(-6).start());
    assert!(api.range(Some("1h"), Some("2h"), None).is_err());
}
