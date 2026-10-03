use otelo_indexed_storage::TimeRange;
use otelo_indexed_storage::query::Resolution;

const HOUR: i64 = 3600 * 1_000_000_000;
const DAY: i64 = 24 * HOUR;
const NOW: i64 = 400 * DAY;

fn choose_for_range_of(length_ns: i64) -> Resolution {
    let range = TimeRange::new(NOW - length_ns, NOW).unwrap();
    Resolution::choose_for_range_length(range)
}

#[test]
fn a_short_range_reads_the_raw_points() {
    assert_eq!(choose_for_range_of(HOUR), Resolution::Raw);
    assert_eq!(choose_for_range_of(6 * HOUR), Resolution::Raw);
}

#[test]
fn a_longer_range_reads_the_summaries_by_the_minute() {
    assert_eq!(choose_for_range_of(7 * HOUR), Resolution::Minute);
    assert_eq!(choose_for_range_of(14 * DAY), Resolution::Minute);
}

#[test]
fn the_longest_range_reads_the_summaries_by_the_hour() {
    assert_eq!(choose_for_range_of(15 * DAY), Resolution::Hour);
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
