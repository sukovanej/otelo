use siner_telemetry::Day;

#[test]
fn names_the_file_by_the_utc_date() {
    let day = Day::of(1_790_000_000 * 1_000_000_000);
    assert_eq!(day.file_name(), "2026-09-21.sqlite");
    assert_eq!(Day::from_file_name("2026-09-21.sqlite"), Some(day));
    assert_eq!(Day::from_file_name("2026-09-21.sqlite-wal"), Some(day));
    assert_eq!(Day::from_file_name("state.sqlite"), None);
}

#[test]
fn puts_a_timestamp_before_1970_on_the_day_before() {
    assert_eq!(Day::of(-1).to_string(), "1969-12-31");
    assert_eq!(Day::of(0).to_string(), "1970-01-01");
}
