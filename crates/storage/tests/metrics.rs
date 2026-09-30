use otelo_storage::query::choose_default_step_ns;
use otelo_storage::{MetricKind, Temporality, TimeRange};

const SECOND: i64 = 1_000_000_000;

#[test]
fn picks_a_step_that_fits_the_range() {
    let step_until = |end_at: i64| choose_default_step_ns(TimeRange::new(0, end_at).unwrap());
    assert_eq!(step_until(3600 * SECOND), 30 * SECOND);
    assert_eq!(step_until(60 * SECOND), SECOND);
    assert_eq!(step_until(7 * 86_400 * SECOND), 3 * 3600 * SECOND);
    assert_eq!(step_until(1000 * 86_400 * SECOND), 86_400 * SECOND);
}

#[test]
fn a_metric_kind_is_stored_and_sent_under_its_name() {
    for kind in [
        MetricKind::Gauge,
        MetricKind::UpDown,
        MetricKind::Counter(Temporality::Cumulative),
        MetricKind::Counter(Temporality::Delta),
        MetricKind::Histogram(Temporality::Cumulative),
        MetricKind::Histogram(Temporality::Delta),
    ] {
        let temporality = kind.temporality().map(Temporality::name);
        assert_eq!(
            MetricKind::from_name_and_temporality(kind.name(), temporality),
            Some(kind)
        );
        let sent = serde_json::to_value(kind).unwrap();
        assert_eq!(sent["kind"], kind.name());
        assert_eq!(
            sent.get("temporality").and_then(|sent| sent.as_str()),
            temporality
        );
    }
    assert_eq!(MetricKind::from_name_and_temporality("summary", None), None);
    assert_eq!(
        MetricKind::from_name_and_temporality("gauge", Some("delta")),
        None
    );
    assert_eq!(MetricKind::from_name_and_temporality("counter", None), None);
}
