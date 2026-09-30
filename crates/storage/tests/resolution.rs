use otelo_storage::query::Resolution;
use otelo_storage::{MetricRetention, TimeRange};

const HOUR: i64 = 3600 * 1_000_000_000;
const DAY: i64 = 24 * HOUR;
const NOW: i64 = 400 * DAY;

const RETENTION: MetricRetention = MetricRetention {
    oldest_raw_at: NOW - 7 * DAY,
    oldest_minute_at: NOW - 14 * DAY,
    oldest_hour_at: NOW - 90 * DAY,
};

fn finest_kept(ago: i64, length: i64) -> Resolution {
    let range = TimeRange::new(NOW - ago, NOW - ago + length).unwrap();
    Resolution::finest_kept_for(range, RETENTION)
}

#[test]
fn a_short_range_reads_the_raw_points() {
    assert_eq!(finest_kept(HOUR, HOUR), Resolution::Raw);
    assert_eq!(finest_kept(6 * HOUR, 6 * HOUR), Resolution::Raw);
    assert_eq!(finest_kept(6 * DAY, HOUR), Resolution::Raw);
}

#[test]
fn a_longer_range_reads_the_summaries_by_the_minute() {
    assert_eq!(finest_kept(7 * HOUR, 7 * HOUR), Resolution::Minute);
    assert_eq!(finest_kept(14 * DAY, 14 * DAY), Resolution::Minute);
}

#[test]
fn the_longest_range_reads_the_summaries_by_the_hour() {
    assert_eq!(finest_kept(30 * DAY, 30 * DAY), Resolution::Hour);
}

#[test]
fn a_range_that_starts_before_the_finer_points_are_kept_reads_the_coarser_ones() {
    assert_eq!(finest_kept(10 * DAY, HOUR), Resolution::Minute);
    assert_eq!(finest_kept(20 * DAY, HOUR), Resolution::Hour);
    assert_eq!(finest_kept(20 * DAY, 2 * DAY), Resolution::Hour);
}

#[test]
fn a_resolution_has_the_name_the_api_takes() {
    for resolution in [Resolution::Raw, Resolution::Minute, Resolution::Hour] {
        assert_eq!(resolution.name().parse(), Ok(resolution));
        assert_eq!(
            serde_json::to_string(&resolution).unwrap(),
            format!("\"{}\"", resolution.name())
        );
    }
    assert!("5m".parse::<Resolution>().is_err());
}
