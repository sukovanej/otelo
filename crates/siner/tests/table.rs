use siner::table::{duration, number};

#[test]
fn formats_durations() {
    assert_eq!(duration(820), "820ns");
    assert_eq!(duration(820_000), "820µs");
    assert_eq!(duration(35_400_000), "35.4ms");
    assert_eq!(duration(1_250_000_000), "1.25s");
    assert_eq!(duration(0), "0s");
    assert_eq!(duration(185_000_000_000), "3m05s");
    assert_eq!(duration(1_800_000_000_000), "30m");
    assert_eq!(duration(9_000_000_000_000), "2h30m");
}

#[test]
fn formats_numbers() {
    assert_eq!(number(100.0), "100");
    assert_eq!(number(0.123_456), "0.123");
    assert_eq!(number(-0.000_1), "0");
}
