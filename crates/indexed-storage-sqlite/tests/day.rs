use otelo_indexed_storage_sqlite::Day;

#[test]
fn a_day_is_its_utc_date() {
    let day = Day::from_unix_nanos(1_790_000_000 * 1_000_000_000);
    assert_eq!(day.to_string(), "2026-09-21");
    assert_eq!("2026-09-21".parse::<Day>().unwrap(), day);
    assert_eq!(day.add_days(1).to_string(), "2026-09-22");
}

#[test]
fn puts_a_timestamp_before_1970_on_the_day_before() {
    assert_eq!(Day::from_unix_nanos(-1).to_string(), "1969-12-31");
    assert_eq!(Day::from_unix_nanos(0).to_string(), "1970-01-01");
}
