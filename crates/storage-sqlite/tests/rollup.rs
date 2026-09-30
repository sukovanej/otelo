use std::path::Path;

use otelo_query::{Signal, parse};
use otelo_storage::query::{Bucket, BucketChange, MetricFilter, MetricSeries, Resolution};
use otelo_storage::{
    Attributes, Batch, Buckets, ExplicitBuckets, ExponentialBuckets, Histogram, HistogramPoint,
    IndexedCounts, Metric, MetricKind, NumberPoint, Points, RangeQueries, Records, Resource,
    Temporality, TimeRange, batch_channel,
};
use otelo_storage_sqlite::{Config, Day, Reader, RollupProgress, Rollups, Writer};
use rusqlite::Connection;

const SECOND: i64 = 1_000_000_000;
const MINUTE: i64 = 60 * SECOND;
const HOUR: i64 = 60 * MINUTE;

// The writer rolls up what is due when it runs. Points of tomorrow are due later than that, so
// only the test rolls them up, at the time it says.
fn ten_tomorrow() -> i64 {
    Day::today().plus(1).start() + 10 * HOUR
}

fn oldest_raw_at() -> i64 {
    Day::today().plus(-6).start()
}

fn write_metrics(dir: &Path, metrics: Vec<Metric>) {
    let batch: Batch = vec![Records {
        resource: Resource {
            service: "api".into(),
            attributes: serde_json::from_value(serde_json::json!({"service.name": "api"})).unwrap(),
        },
        logs: Vec::new(),
        spans: Vec::new(),
        metrics,
    }];
    let (sender, inbox) = batch_channel(1);
    assert!(sender.send(batch));
    let writer = Writer::spawn(Config::new(dir.to_owned()), inbox).unwrap();
    drop(sender);
    writer.join().unwrap();
}

fn roll_up_all_due(dir: &Path, now: i64) -> usize {
    let mut rollups = Rollups::open(dir).unwrap();
    let mut passes = 1;
    while matches!(
        rollups.roll_up_next_due(now, oldest_raw_at()).unwrap(),
        RollupProgress::MoreIsDue
    ) {
        passes += 1;
        assert!(passes < 1000, "the rollups never finish");
    }
    passes
}

fn metric(name: &str, points: Points) -> Metric {
    Metric {
        name: name.into(),
        unit: "1".into(),
        labels: Attributes::new(),
        points,
    }
}

fn numbers(points: &[(i64, f64)]) -> Vec<NumberPoint> {
    points
        .iter()
        .map(|&(second, value)| NumberPoint {
            recorded_at: ten_tomorrow() + second * SECOND,
            value,
        })
        .collect()
}

fn explicit(second: i64, counts: &[u64], sum: f64) -> HistogramPoint {
    HistogramPoint {
        recorded_at: ten_tomorrow() + second * SECOND,
        histogram: Histogram {
            count: counts.iter().sum(),
            sum: Some(sum),
            min: None,
            max: None,
            buckets: Buckets::Explicit(ExplicitBuckets {
                bounds: vec![0.5],
                counts: counts.to_vec(),
            }),
        },
    }
}

fn exponential(second: i64, scale: i32, counts: &[u64]) -> HistogramPoint {
    HistogramPoint {
        recorded_at: ten_tomorrow() + second * SECOND,
        histogram: Histogram {
            count: counts.iter().sum(),
            sum: Some(1.0),
            min: None,
            max: None,
            buckets: Buckets::Exponential(ExponentialBuckets {
                scale,
                zero_count: 0,
                positive: IndexedCounts {
                    offset: 0,
                    counts: counts.to_vec(),
                },
                negative: IndexedCounts::default(),
            }),
        },
    }
}

fn each_kind() -> Vec<Metric> {
    vec![
        metric(
            "queue.lag",
            Points::Gauge(numbers(&[(5, 1.0), (20, 5.0), (65, 3.0)])),
        ),
        metric(
            "emails.sent",
            Points::Counter(
                Temporality::Cumulative,
                // The last value fell: the app started again.
                numbers(&[(0, 100.0), (30, 130.0), (60, 190.0), (90, 20.0)]),
            ),
        ),
        metric(
            "bytes.sent",
            Points::Counter(
                Temporality::Delta,
                numbers(&[(10, 5.0), (40, 7.0), (70, 9.0)]),
            ),
        ),
        metric(
            "request.duration",
            Points::Histogram(
                Temporality::Cumulative,
                vec![
                    explicit(15, &[1, 0], 1.0),
                    explicit(45, &[3, 1], 9.0),
                    explicit(75, &[4, 1], 9.5),
                ],
            ),
        ),
        metric(
            "query.duration",
            Points::Histogram(
                Temporality::Delta,
                vec![
                    exponential(15, 0, &[2, 2]),
                    exponential(45, 1, &[1, 1, 1, 1]),
                ],
            ),
        ),
    ]
}

fn read_metric(dir: &Path, name: &str, resolution: Resolution, step_ns: i64) -> MetricSeries {
    let range = TimeRange::new(ten_tomorrow(), ten_tomorrow() + 4 * HOUR).unwrap();
    let reader = Reader::open(dir, range).unwrap();
    let filter = MetricFilter {
        name: name.into(),
        query: parse("", Signal::Metrics).unwrap(),
        step_ns,
        resolution,
    };
    reader.metric(&filter, 10).unwrap()
}

fn buckets(dir: &Path, name: &str, resolution: Resolution) -> Vec<Bucket> {
    let step = match resolution {
        Resolution::Hour => HOUR,
        Resolution::Raw | Resolution::Minute => MINUTE,
    };
    let mut metric = read_metric(dir, name, resolution, step);
    assert_eq!(metric.series.len(), 1, "{name}");
    metric.series.remove(0).buckets
}

fn levels(buckets: &[Bucket]) -> Vec<(u64, f64, f64, f64, f64)> {
    buckets
        .iter()
        .map(|b| (b.count, b.min, b.max, b.avg, b.last))
        .collect()
}

fn rates(buckets: &[Bucket]) -> Vec<Option<f64>> {
    buckets
        .iter()
        .map(|bucket| match bucket.change {
            BucketChange::Rate { per_second } => Some(per_second),
            BucketChange::None | BucketChange::Distribution(_) => None,
        })
        .collect()
}

fn counts(buckets: &[Bucket]) -> Vec<Option<Vec<u64>>> {
    buckets
        .iter()
        .map(|bucket| match &bucket.change {
            BucketChange::Distribution(distribution) => Some(distribution.counts.clone()),
            BucketChange::None | BucketChange::Rate { .. } => None,
        })
        .collect()
}

#[test]
fn a_minute_sums_up_each_kind_as_the_raw_points_do() {
    let dir = tempfile::tempdir().unwrap();
    write_metrics(dir.path(), each_kind());
    roll_up_all_due(dir.path(), ten_tomorrow() + 10 * MINUTE);

    for name in [
        "queue.lag",
        "emails.sent",
        "bytes.sent",
        "request.duration",
        "query.duration",
    ] {
        let raw = buckets(dir.path(), name, Resolution::Raw);
        let minutes = buckets(dir.path(), name, Resolution::Minute);
        assert_eq!(levels(&minutes), levels(&raw), "{name}");
        assert_eq!(rates(&minutes), rates(&raw), "{name}");
        assert_eq!(counts(&minutes), counts(&raw), "{name}");
    }

    let lag = buckets(dir.path(), "queue.lag", Resolution::Minute);
    assert_eq!(
        levels(&lag),
        [(2, 1.0, 5.0, 3.0, 5.0), (1, 3.0, 3.0, 3.0, 3.0)]
    );
    let sent = buckets(dir.path(), "emails.sent", Resolution::Minute);
    // The second minute counts 60 up to 190, and 20 from zero after the restart.
    assert_eq!(rates(&sent), [Some(1.0), Some(80.0 / 60.0)]);
    let bytes = buckets(dir.path(), "bytes.sent", Resolution::Minute);
    assert_eq!(rates(&bytes), [Some(12.0 / 30.0), Some(9.0 / 30.0)]);
    let request = buckets(dir.path(), "request.duration", Resolution::Minute);
    assert_eq!(counts(&request), [Some(vec![2, 1]), Some(vec![1, 0])]);
    let query = buckets(dir.path(), "query.duration", Resolution::Minute);
    // The point of scale 1 joins its buckets in pairs.
    assert_eq!(counts(&query), [Some(vec![0, 4, 4, 0])]);
}

#[test]
fn an_hour_adds_up_its_minutes() {
    let dir = tempfile::tempdir().unwrap();
    write_metrics(dir.path(), each_kind());
    // The hour is due once the minutes of all of it are rolled up.
    roll_up_all_due(dir.path(), ten_tomorrow() + 30 * MINUTE);
    assert!(
        read_metric(dir.path(), "queue.lag", Resolution::Hour, HOUR)
            .series
            .is_empty()
    );
    roll_up_all_due(dir.path(), ten_tomorrow() + HOUR + 5 * MINUTE);

    let lag = buckets(dir.path(), "queue.lag", Resolution::Hour);
    assert_eq!(levels(&lag), [(3, 1.0, 5.0, 3.0, 3.0)]);
    let sent = buckets(dir.path(), "emails.sent", Resolution::Hour);
    assert_eq!(rates(&sent), [Some(110.0 / 90.0)]);
    let request = buckets(dir.path(), "request.duration", Resolution::Hour);
    assert_eq!(counts(&request), [Some(vec![3, 1])]);
    let hour = read_metric(dir.path(), "request.duration", Resolution::Hour, HOUR);
    assert_eq!(
        hour.series[0].kind,
        MetricKind::Histogram(Temporality::Delta)
    );
    assert_eq!(hour.resolution, Resolution::Hour);
}

// Without the counters the writer sends about itself when it stops.
fn rows_of(dir: &Path, table: &str) -> Vec<String> {
    let conn = Connection::open(dir.join("metrics-rollup.sqlite")).unwrap();
    let sql = format!(
        "SELECT json_array(s.name, m.start, m.count, m.min, m.max, m.sum, m.last, m.increase,
                           m.seconds, m.histogram)
         FROM {table} m JOIN series s ON s.id = m.series_id
         WHERE s.name NOT LIKE 'otelo.%' ORDER BY s.name, m.start"
    );
    conn.prepare(&sql)
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn rolling_a_minute_up_again_gives_the_same_rows() {
    let dir = tempfile::tempdir().unwrap();
    write_metrics(dir.path(), each_kind());
    let now = ten_tomorrow() + HOUR + 5 * MINUTE;
    roll_up_all_due(dir.path(), now);
    let (minutes, hours) = (rows_of(dir.path(), "minutes"), rows_of(dir.path(), "hours"));
    assert_eq!(minutes.len(), 9);
    assert_eq!(hours.len(), 5);

    Connection::open(dir.path().join("metrics-rollup.sqlite"))
        .unwrap()
        .execute("DELETE FROM cursors", [])
        .unwrap();
    roll_up_all_due(dir.path(), now);
    assert_eq!(rows_of(dir.path(), "minutes"), minutes);
    assert_eq!(rows_of(dir.path(), "hours"), hours);
}

#[test]
fn fills_the_hours_it_missed_while_the_daemon_was_down() {
    let dir = tempfile::tempdir().unwrap();
    write_metrics(
        dir.path(),
        vec![metric(
            "queue.lag",
            Points::Gauge(numbers(&[(5, 1.0), (3 * 3600 + 5, 7.0)])),
        )],
    );
    roll_up_all_due(dir.path(), ten_tomorrow() + 5 * MINUTE);
    assert_eq!(
        levels(&buckets(dir.path(), "queue.lag", Resolution::Minute)),
        [(1, 1.0, 1.0, 1.0, 1.0)]
    );

    // Four hours later the job rolls up an hour at a time until nothing is due.
    let passes = roll_up_all_due(dir.path(), ten_tomorrow() + 4 * HOUR + 5 * MINUTE);
    assert!(passes >= 4, "{passes} passes");
    assert_eq!(
        levels(&buckets(dir.path(), "queue.lag", Resolution::Minute)),
        [(1, 1.0, 1.0, 1.0, 1.0), (1, 7.0, 7.0, 7.0, 7.0)]
    );
    assert_eq!(
        levels(&buckets(dir.path(), "queue.lag", Resolution::Hour)),
        [(1, 1.0, 1.0, 1.0, 1.0), (1, 7.0, 7.0, 7.0, 7.0)]
    );
}

#[test]
fn a_step_of_summaries_is_a_whole_number_of_them() {
    let dir = tempfile::tempdir().unwrap();
    write_metrics(dir.path(), each_kind());
    roll_up_all_due(dir.path(), ten_tomorrow() + 10 * MINUTE);
    let metric = read_metric(dir.path(), "queue.lag", Resolution::Minute, 90 * SECOND);
    assert_eq!(metric.step_ns, 2 * MINUTE);
    assert_eq!(levels(&metric.series[0].buckets), [(3, 1.0, 5.0, 3.0, 3.0)]);
}

#[test]
fn lists_the_series_that_have_summaries_in_the_range() {
    let dir = tempfile::tempdir().unwrap();
    write_metrics(dir.path(), each_kind());
    roll_up_all_due(dir.path(), ten_tomorrow() + 10 * MINUTE);
    let list = |since: i64, query: &str| {
        let range = TimeRange::new(since, since + HOUR).unwrap();
        let reader = Reader::open(dir.path(), range).unwrap();
        let query = parse(query, Signal::Metrics).unwrap();
        let list = reader.metrics(&query, Resolution::Minute, 10).unwrap();
        list.series
            .into_iter()
            .map(|series| format!("{} {}", series.name, series.kind.name()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        list(ten_tomorrow() + 30 * SECOND, ""),
        [
            "bytes.sent counter",
            "emails.sent counter",
            "query.duration histogram",
            "queue.lag gauge",
            "request.duration histogram",
        ]
    );
    assert_eq!(
        list(ten_tomorrow(), "kind = gauge service = api"),
        ["queue.lag gauge"]
    );
    assert!(list(ten_tomorrow() + 2 * HOUR, "").is_empty());
}

#[test]
fn deletes_the_summaries_past_their_retention_and_the_series_without_any() {
    let dir = tempfile::tempdir().unwrap();
    write_metrics(dir.path(), each_kind());
    roll_up_all_due(dir.path(), ten_tomorrow() + HOUR + 5 * MINUTE);
    let mut rollups = Rollups::open(dir.path()).unwrap();

    // The second minute and the hour are still kept.
    rollups
        .delete_summaries_before(ten_tomorrow() + MINUTE, ten_tomorrow())
        .unwrap();
    assert_eq!(rows_of(dir.path(), "minutes").len(), 4);
    assert_eq!(rows_of(dir.path(), "hours").len(), 5);

    rollups
        .delete_summaries_before(ten_tomorrow() + HOUR, ten_tomorrow() + HOUR)
        .unwrap();
    let conn = Connection::open(dir.path().join("metrics-rollup.sqlite")).unwrap();
    let left: i64 = conn
        .query_row(
            "SELECT (SELECT count(*) FROM series) + (SELECT count(*) FROM resources)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(left, 0);
}
