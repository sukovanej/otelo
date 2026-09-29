use otelo::cli::table::{format_duration, format_number};

#[test]
fn formats_durations() {
    assert_eq!(format_duration(820), "820ns");
    assert_eq!(format_duration(820_000), "820µs");
    assert_eq!(format_duration(35_400_000), "35.4ms");
    assert_eq!(format_duration(1_250_000_000), "1.25s");
    assert_eq!(format_duration(0), "0s");
    assert_eq!(format_duration(185_000_000_000), "3m05s");
    assert_eq!(format_duration(1_800_000_000_000), "30m");
    assert_eq!(format_duration(9_000_000_000_000), "2h30m");
}

#[test]
fn formats_numbers() {
    assert_eq!(format_number(100.0), "100");
    assert_eq!(format_number(0.123_456), "0.123");
    assert_eq!(format_number(-0.000_1), "0");
}
