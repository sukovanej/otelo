use std::num::NonZeroUsize;

use otelo_indexed_storage::query::{BucketChange, GroupKey, Grouping, GroupingField, SeriesGroup};
use otelo_indexed_storage::{
    Attributes, Buckets, ExplicitBuckets, Histogram, HistogramPoint, MetricKind, NumberPoint,
    SeriesPoint, SeriesSteps, SummarizedSeries, Temporality, group_series,
};
use serde_json::{Value, json};

const SECOND: i64 = 1_000_000_000;
const STEP_NS: i64 = 60 * SECOND;

fn attributes_from_json(value: Value) -> Attributes {
    serde_json::from_value(value).unwrap()
}

struct SeriesOf {
    service: &'static str,
    kind: MetricKind,
    unit: &'static str,
    labels: Value,
    resource: Value,
}

impl SeriesOf {
    fn labelled(kind: MetricKind, labels: Value) -> Self {
        Self {
            service: "api",
            kind,
            unit: "By",
            labels,
            resource: json!({}),
        }
    }

    fn summarize(self, points: Vec<SeriesPoint>) -> SummarizedSeries {
        let mut steps = SeriesSteps::new(self.kind);
        for point in points {
            let step_start_at = point.recorded_at().div_euclid(STEP_NS) * STEP_NS;
            steps.add_point(step_start_at, point);
        }
        SummarizedSeries {
            service: self.service.into(),
            kind: self.kind,
            unit: self.unit.into(),
            labels: attributes_from_json(self.labels),
            resource: attributes_from_json(self.resource),
            summaries_by_step: steps.into_summaries_by_step(),
        }
    }
}

fn number_points(points: &[(i64, f64)]) -> Vec<SeriesPoint> {
    points
        .iter()
        .map(|&(second, value)| {
            SeriesPoint::Number(NumberPoint {
                recorded_at: second * SECOND,
                value,
            })
        })
        .collect()
}

fn histogram_point(second: i64, counts: &[u64], sum: f64) -> SeriesPoint {
    SeriesPoint::Histogram(HistogramPoint {
        recorded_at: second * SECOND,
        histogram: Histogram {
            count: counts.iter().sum(),
            sum: Some(sum),
            min: None,
            max: None,
            buckets: Buckets::Explicit(
                ExplicitBuckets::new(vec![0.1, 1.0], counts.to_vec()).unwrap(),
            ),
        },
    })
}

fn grouping(by: &[&str], top: Option<usize>) -> Grouping {
    Grouping {
        by: by.iter().map(|name| name.parse().unwrap()).collect(),
        top: top.map(|top| NonZeroUsize::new(top).unwrap()),
    }
}

fn levels(group: &SeriesGroup) -> Vec<(u64, f64, f64, f64, f64)> {
    group
        .buckets
        .iter()
        .map(|bucket| {
            (
                bucket.count,
                bucket.min,
                bucket.max,
                bucket.avg,
                bucket.last,
            )
        })
        .collect()
}

fn rates(group: &SeriesGroup) -> Vec<Option<f64>> {
    group
        .buckets
        .iter()
        .map(|bucket| match bucket.change {
            BucketChange::Rate { per_second } => Some(per_second),
            BucketChange::None | BucketChange::Distribution(_) => None,
        })
        .collect()
}

fn values_group(values: Value, series_count: usize) -> GroupKey {
    GroupKey::Values {
        values: attributes_from_json(values),
        series_count,
    }
}

#[test]
fn a_gauge_group_takes_the_average_of_its_series() {
    let series = vec![
        SeriesOf::labelled(MetricKind::Gauge, json!({"cpu": 0, "cpu.mode": "user"}))
            .summarize(number_points(&[(0, 0.25), (30, 0.75)])),
        SeriesOf::labelled(MetricKind::Gauge, json!({"cpu": 1, "cpu.mode": "user"}))
            .summarize(number_points(&[(10, 0.5)])),
    ];
    let groups = group_series(series, &grouping(&["cpu.mode"], None));
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].key, values_group(json!({"cpu.mode": "user"}), 2));
    assert_eq!(levels(&groups[0]), [(3, 0.25, 0.75, 0.5, 0.625)]);
}

#[test]
fn an_updown_group_adds_up_its_series() {
    let series = vec![
        SeriesOf::labelled(MetricKind::UpDown, json!({"state": "used", "host": "a"}))
            .summarize(number_points(&[(0, 100.0), (30, 300.0), (60, 200.0)])),
        SeriesOf::labelled(MetricKind::UpDown, json!({"state": "used", "host": "b"}))
            .summarize(number_points(&[(10, 50.0)])),
    ];
    let groups = group_series(series, &grouping(&["state"], None));
    assert_eq!(groups.len(), 1);
    // The second step has no point of host b, so it holds host a alone.
    assert_eq!(
        levels(&groups[0]),
        [
            (3, 150.0, 350.0, 250.0, 350.0),
            (1, 200.0, 200.0, 200.0, 200.0)
        ]
    );
}

#[test]
fn a_counter_group_adds_up_the_rates_of_its_series() {
    let counter = MetricKind::Counter(Temporality::Cumulative);
    let series = vec![
        SeriesOf::labelled(counter, json!({"interface": "eth0"})).summarize(number_points(&[
            (0, 100.0),
            (30, 130.0),
            (90, 190.0),
        ])),
        SeriesOf::labelled(counter, json!({"interface": "eth1"}))
            .summarize(number_points(&[(0, 0.0), (20, 40.0)])),
    ];
    let groups = group_series(series, &grouping(&["service"], None));
    assert_eq!(groups[0].key, values_group(json!({"service": "api"}), 2));
    assert_eq!(rates(&groups[0]), [Some(3.0), Some(1.0)]);
}

#[test]
fn a_histogram_group_merges_the_buckets_of_its_series() {
    let histogram = MetricKind::Histogram(Temporality::Delta);
    let series = vec![
        SeriesOf::labelled(
            histogram,
            json!({"http.route": "/matches", "method": "GET"}),
        )
        .summarize(vec![histogram_point(0, &[1, 2, 0], 1.5)]),
        SeriesOf::labelled(
            histogram,
            json!({"http.route": "/matches", "method": "POST"}),
        )
        .summarize(vec![histogram_point(10, &[3, 0, 1], 2.0)]),
    ];
    let groups = group_series(series, &grouping(&["http.route"], None));
    let BucketChange::Distribution(distribution) = &groups[0].buckets[0].change else {
        panic!("a distribution, not {:?}", groups[0].buckets[0].change);
    };
    assert_eq!(distribution.counts, [4, 2, 1]);
    assert_eq!(distribution.count, 7);
    assert_eq!(distribution.sum, Some(3.5));
}

#[test]
fn a_group_holds_the_values_its_series_have() {
    let series_on_host = |host: Option<&str>, labels: Value| SeriesOf {
        service: "api",
        kind: MetricKind::Gauge,
        unit: "s",
        labels,
        resource: host.map_or_else(|| json!({}), |host| json!({"host.name": host})),
    };
    let series = vec![
        series_on_host(Some("droplet"), json!({"http.route": "/matches"}))
            .summarize(number_points(&[(0, 3.0)])),
        series_on_host(None, json!({"http.route": "/matches"}))
            .summarize(number_points(&[(0, 2.0)])),
        series_on_host(Some("droplet"), json!({"service": "mudro"}))
            .summarize(number_points(&[(0, 1.0)])),
    ];
    let groups = group_series(
        series,
        &grouping(&["http.route", "resource.host.name", "attr.service"], None),
    );
    let keys: Vec<&GroupKey> = groups.iter().map(|group| &group.key).collect();
    assert_eq!(
        keys,
        [
            &values_group(
                json!({"http.route": "/matches", "resource.host.name": "droplet"}),
                1
            ),
            &values_group(json!({"http.route": "/matches"}), 1),
            &values_group(
                json!({"resource.host.name": "droplet", "attr.service": "mudro"}),
                1
            ),
        ]
    );
}

#[test]
fn top_keeps_the_highest_groups_and_combines_the_rest_as_other() {
    let series = [
        ("a", 10.0),
        ("b", 50.0),
        ("c", 30.0),
        ("d", 40.0),
        ("e", 20.0),
    ]
    .into_iter()
    .map(|(service, value)| {
        SeriesOf {
            service,
            ..SeriesOf::labelled(MetricKind::UpDown, json!({}))
        }
        .summarize(number_points(&[(0, value)]))
    })
    .collect();
    let groups = group_series(series, &grouping(&[], Some(3)));
    let described: Vec<(String, f64)> = groups
        .iter()
        .map(|group| {
            let name = match &group.key {
                GroupKey::Series { service, .. } => service.clone(),
                GroupKey::Other {
                    group_count,
                    series_count,
                } => format!("other of {group_count} groups, {series_count} series"),
                GroupKey::Values { .. } => panic!("no group by values without by"),
            };
            (name, group.buckets[0].avg)
        })
        .collect();
    assert_eq!(
        described,
        [
            ("b".to_owned(), 50.0),
            ("d".to_owned(), 40.0),
            ("c".to_owned(), 30.0),
            ("other of 2 groups, 2 series".to_owned(), 30.0),
        ]
    );
}

#[test]
fn a_histogram_group_ranks_by_the_sum_of_its_values() {
    let histogram = MetricKind::Histogram(Temporality::Delta);
    let series = vec![
        SeriesOf::labelled(histogram, json!({"http.route": "/matches"}))
            .summarize(vec![histogram_point(0, &[1000, 0, 0], 20.0)]),
        SeriesOf::labelled(histogram, json!({"http.route": "/replay"}))
            .summarize(vec![histogram_point(0, &[0, 0, 10], 30.0)]),
    ];
    let groups = group_series(series, &grouping(&["http.route"], Some(1)));
    assert_eq!(
        groups[0].key,
        values_group(json!({"http.route": "/replay"}), 1)
    );
    assert_eq!(
        groups[1].key,
        GroupKey::Other {
            group_count: 1,
            series_count: 1
        }
    );
}

#[test]
fn series_of_other_kinds_or_units_stay_apart() {
    let series = [("a", "By", 10.0), ("b", "By", 20.0), ("c", "KiBy", 5.0)]
        .into_iter()
        .map(|(service, unit, value)| {
            SeriesOf {
                service,
                unit,
                ..SeriesOf::labelled(MetricKind::UpDown, json!({}))
            }
            .summarize(number_points(&[(0, value)]))
        })
        .collect();
    let groups = group_series(series, &grouping(&[], Some(1)));
    let described: Vec<(&str, &GroupKey)> = groups
        .iter()
        .map(|group| (group.unit.as_str(), &group.key))
        .collect();
    let series_of = |service: &str| GroupKey::Series {
        service: service.into(),
        labels: Attributes::new(),
        resource: Attributes::new(),
    };
    assert_eq!(
        described,
        [
            ("By", &series_of("b")),
            ("KiBy", &series_of("c")),
            (
                "By",
                &GroupKey::Other {
                    group_count: 1,
                    series_count: 1
                }
            ),
        ]
    );
}

#[test]
fn series_group_by_a_label_the_service_or_a_resource_key() {
    let parse = |name: &str| name.parse::<GroupingField>();
    assert_eq!(
        parse("http.route"),
        Ok(GroupingField::Label("http.route".into()))
    );
    assert_eq!(parse("service"), Ok(GroupingField::Service));
    assert_eq!(
        parse("resource.host.name"),
        Ok(GroupingField::Resource("host.name".into()))
    );
    assert_eq!(
        parse("attr.service"),
        Ok(GroupingField::Label("service".into()))
    );
    assert_eq!(
        GroupingField::Label("service".into()).to_string(),
        "attr.service"
    );
    for shared in ["name", "kind", "unit"] {
        assert!(parse(shared).unwrap_err().contains(shared), "{shared}");
    }
}
