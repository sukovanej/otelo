use otelo_journal::Hour;

const NANOS_PER_HOUR: i64 = 3_600 * 1_000_000_000;

#[test]
fn hour_contains_the_instants_from_its_start_to_before_its_end() {
    let hour = Hour::from_hours_since_epoch(10);

    assert_eq!(Hour::containing(10 * NANOS_PER_HOUR), hour);
    assert_eq!(Hour::containing(11 * NANOS_PER_HOUR - 1), hour);
    assert_eq!(hour.started_at(), 10 * NANOS_PER_HOUR);
    assert_eq!(hour.ended_at(), 11 * NANOS_PER_HOUR);
    assert_eq!(hour.next(), Hour::from_hours_since_epoch(11));
}

#[test]
fn hour_before_the_epoch_rounds_down() {
    assert_eq!(Hour::containing(-1), Hour::from_hours_since_epoch(-1));
}
