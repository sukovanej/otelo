use std::path::Path;

use otelo_indexed_storage::query::{
    Bucket, BucketChange, Grouping, MetricFilter, MetricSeries, Resolution,
};
use otelo_indexed_storage::{
    Attributes, Batch, Buckets, ExplicitBuckets, ExponentialBuckets, Histogram, HistogramPoint,
    IndexedCounts, Metric, MetricKind, NumberPoint, Points, RangeQueries, Records, Resource,
    Temporality, TimeRange, open_batch_channel,
};
use otelo_indexed_storage_sqlite::{Config, Day, Reader, RollupProgress, Rollups, Writer};
use otelo_query::{Signal, parse_query};
use rusqlite::Connection;

const SECOND: i64 = 1_000_000_000;
const MINUTE: i64 = 60 * SECOND;
const HOUR: i64 = 60 * MINUTE;

// The writer rolls up what is due when it runs. Points of tomorrow are due later than that, so
// only the test rolls them up, at the time it says.
fn ten_tomorrow() -> i64 {
    Day::today().add_days(1).start_at() + 10 * HOUR
}

fn oldest_raw_at() -> i64 {
    Day::today().add_days(-6).start_at()
}

fn write_metrics(directory: &Path, metrics: Vec<Metric>) {
    let batch: Batch = vec![Records {
        resource: Resource {
            service: "api".into(),
            attributes: serde_json::from_value(serde_json::json!({"service.name": "api"})).unwrap(),
        },
        logs: Vec::new(),
        spans: Vec::new(),
        metrics,
    }];
    let (sender, inbox) = open_batch_channel(1);
    assert!(sender.send_batch(batch));
    let writer = Writer::spawn(Config::new(directory.to_owned()), inbox).unwrap();
    drop(sender);
    writer.join().unwrap();
}

fn roll_up_all_due(directory: &Path, now: i64) -> usize {
    let mut rollups = Rollups::open(directory).unwrap();
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

fn number_points(points: &[(i64, f64)]) -> Vec<NumberPoint> {
    points
        .iter()
        .map(|&(second, value)| NumberPoint {
            recorded_at: ten_tomorrow() + second * SECOND,
            value,
        })
        .collect()
}

fn explicit_point(second: i64, counts: &[u64], sum: f64) -> HistogramPoint {
    HistogramPoint {
        recorded_at: ten_tomorrow() + second * SECOND,
        histogram: Histogram {
            count: counts.iter().sum(),
            sum: Some(sum),
            min: None,
            max: None,
            buckets: Buckets::Explicit(ExplicitBuckets::new(vec![0.5], counts.to_vec()).unwrap()),
        },
    }
}

fn exponential_point(second: i64, scale: i32, counts: &[u64]) -> HistogramPoint {
    HistogramPoint {
        recorded_at: ten_tomorrow() + second * SECOND,
        histogram: Histogram {
            count: counts.iter().sum(),
            sum: Some(1.0),
            min: None,
            max: None,
            buckets: Buckets::Exponential(
                ExponentialBuckets::new(
                    scale,
                    0,
                    IndexedCounts {
                        offset: 0,
                        counts: counts.to_vec(),
                    },
                    IndexedCounts::default(),
                )
                .unwrap(),
            ),
        },
    }
}

fn metrics_of_each_kind() -> Vec<Metric> {
    vec![
        metric(
            "queue.lag",
            Points::Gauge(number_points(&[(5, 1.0), (20, 5.0), (65, 3.0)])),
        ),
        metric(
            "emails.sent",
            Points::Counter(
                Temporality::Cumulative,
                // The last value fell: the app started again.
                number_points(&[(0, 100.0), (30, 130.0), (60, 190.0), (90, 20.0)]),
            ),
        ),
        metric(
            "bytes.sent",
            Points::Counter(
                Temporality::Delta,
                number_points(&[(10, 5.0), (40, 7.0), (70, 9.0)]),
            ),
        ),
        metric(
            "request.duration",
            Points::Histogram(
                Temporality::Cumulative,
                vec![
                    explicit_point(15, &[1, 0], 1.0),
                    explicit_point(45, &[3, 1], 9.0),
                    explicit_point(75, &[4, 1], 9.5),
                ],
            ),
        ),
        metric(
            "query.duration",
            Points::Histogram(
                Temporality::Delta,
                vec![
                    exponential_point(15, 0, &[2, 2]),
                    exponential_point(45, 1, &[1, 1, 1, 1]),
                ],
            ),
        ),
    ]
}

fn read_metric_series(
    directory: &Path,
    name: &str,
    resolution: Resolution,
    step_ns: i64,
) -> MetricSeries {
    read_grouped_metric_series(directory, name, resolution, step_ns, Grouping::default())
}

fn read_grouped_metric_series(
    directory: &Path,
    name: &str,
    resolution: Resolution,
    step_ns: i64,
    grouping: Grouping,
) -> MetricSeries {
    let range = TimeRange::new(ten_tomorrow(), ten_tomorrow() + 4 * HOUR).unwrap();
    let reader = Reader::open(directory, range).unwrap();
    let filter = MetricFilter {
        name: name.into(),
        query: parse_query("", Signal::Metrics).unwrap(),
        step_ns,
        resolution,
        grouping,
    };
    reader.get_metric_series(&filter, 10).unwrap()
}

fn read_buckets(directory: &Path, name: &str, resolution: Resolution) -> Vec<Bucket> {
    let step_ns = match resolution {
        Resolution::Hour => HOUR,
        Resolution::Raw | Resolution::Minute => MINUTE,
    };
    let mut metric_series = read_metric_series(directory, name, resolution, step_ns);
    assert_eq!(metric_series.groups.len(), 1, "{name}");
    metric_series.groups.remove(0).buckets
}

fn levels(buckets: &[Bucket]) -> Vec<(u64, f64, f64, f64, f64)> {
    buckets
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
    let directory = tempfile::tempdir().unwrap();
    write_metrics(directory.path(), metrics_of_each_kind());
    roll_up_all_due(directory.path(), ten_tomorrow() + 10 * MINUTE);

    for name in [
        "queue.lag",
        "emails.sent",
        "bytes.sent",
        "request.duration",
        "query.duration",
    ] {
        let raw_buckets = read_buckets(directory.path(), name, Resolution::Raw);
        let minute_buckets = read_buckets(directory.path(), name, Resolution::Minute);
        assert_eq!(levels(&minute_buckets), levels(&raw_buckets), "{name}");
        assert_eq!(rates(&minute_buckets), rates(&raw_buckets), "{name}");
        assert_eq!(counts(&minute_buckets), counts(&raw_buckets), "{name}");
    }

    let queue_lag = read_buckets(directory.path(), "queue.lag", Resolution::Minute);
    assert_eq!(
        levels(&queue_lag),
        [(2, 1.0, 5.0, 3.0, 5.0), (1, 3.0, 3.0, 3.0, 3.0)]
    );
    let emails_sent = read_buckets(directory.path(), "emails.sent", Resolution::Minute);
    // The second minute counts 60 up to 190, and 20 from zero after the restart.
    assert_eq!(rates(&emails_sent), [Some(1.0), Some(80.0 / 60.0)]);
    let bytes_sent = read_buckets(directory.path(), "bytes.sent", Resolution::Minute);
    assert_eq!(rates(&bytes_sent), [Some(12.0 / 30.0), Some(9.0 / 30.0)]);
    let request_duration = read_buckets(directory.path(), "request.duration", Resolution::Minute);
    assert_eq!(
        counts(&request_duration),
        [Some(vec![2, 1]), Some(vec![1, 0])]
    );
    let query_duration = read_buckets(directory.path(), "query.duration", Resolution::Minute);
    // The point of scale 1 joins its buckets in pairs.
    assert_eq!(counts(&query_duration), [Some(vec![0, 4, 4, 0])]);
}

#[test]
fn a_group_of_minutes_combines_as_the_group_of_raw_points_does() {
    let directory = tempfile::tempdir().unwrap();
    let queue_metric = |name: &str, queue: &str, points: Points| Metric {
        labels: serde_json::from_value(serde_json::json!({"queue": queue})).unwrap(),
        ..metric(name, points)
    };
    write_metrics(
        directory.path(),
        vec![
            queue_metric(
                "queue.depth",
                "email",
                Points::UpDown(number_points(&[(5, 4.0), (20, 6.0), (65, 2.0)])),
            ),
            queue_metric(
                "queue.depth",
                "sms",
                Points::UpDown(number_points(&[(10, 1.0), (70, 3.0)])),
            ),
            queue_metric(
                "queue.jobs",
                "email",
                Points::Counter(
                    Temporality::Cumulative,
                    number_points(&[(0, 10.0), (30, 40.0), (90, 100.0)]),
                ),
            ),
            queue_metric(
                "queue.jobs",
                "sms",
                Points::Counter(
                    Temporality::Cumulative,
                    number_points(&[(0, 0.0), (60, 120.0)]),
                ),
            ),
        ],
    );
    roll_up_all_due(directory.path(), ten_tomorrow() + 10 * MINUTE);
    let read_group = |name: &str, resolution: Resolution| {
        let grouping = Grouping {
            by: vec!["service".parse().unwrap()],
            top: None,
        };
        let mut metric_series =
            read_grouped_metric_series(directory.path(), name, resolution, MINUTE, grouping);
        assert_eq!(metric_series.groups.len(), 1, "{name}");
        metric_series.groups.remove(0)
    };
    for name in ["queue.depth", "queue.jobs"] {
        let raw_group = read_group(name, Resolution::Raw);
        let minute_group = read_group(name, Resolution::Minute);
        assert_eq!(minute_group.key, raw_group.key, "{name}");
        assert_eq!(
            levels(&minute_group.buckets),
            levels(&raw_group.buckets),
            "{name}"
        );
        assert_eq!(
            rates(&minute_group.buckets),
            rates(&raw_group.buckets),
            "{name}"
        );
    }
    let queue_depth = read_group("queue.depth", Resolution::Minute);
    assert_eq!(
        levels(&queue_depth.buckets),
        [(3, 5.0, 7.0, 6.0, 7.0), (2, 5.0, 5.0, 5.0, 5.0)]
    );
    let queue_jobs = read_group("queue.jobs", Resolution::Minute);
    assert_eq!(rates(&queue_jobs.buckets), [Some(1.0), Some(3.0)]);
}

#[test]
fn an_hour_adds_up_its_minutes() {
    let directory = tempfile::tempdir().unwrap();
    write_metrics(directory.path(), metrics_of_each_kind());
    // The hour is due once the minutes of all of it are rolled up.
    roll_up_all_due(directory.path(), ten_tomorrow() + 30 * MINUTE);
    assert!(
        read_metric_series(directory.path(), "queue.lag", Resolution::Hour, HOUR)
            .groups
            .is_empty()
    );
    roll_up_all_due(directory.path(), ten_tomorrow() + HOUR + 5 * MINUTE);

    let queue_lag = read_buckets(directory.path(), "queue.lag", Resolution::Hour);
    assert_eq!(levels(&queue_lag), [(3, 1.0, 5.0, 3.0, 3.0)]);
    let emails_sent = read_buckets(directory.path(), "emails.sent", Resolution::Hour);
    assert_eq!(rates(&emails_sent), [Some(110.0 / 90.0)]);
    let request_duration = read_buckets(directory.path(), "request.duration", Resolution::Hour);
    assert_eq!(counts(&request_duration), [Some(vec![3, 1])]);
    let hour_series =
        read_metric_series(directory.path(), "request.duration", Resolution::Hour, HOUR);
    assert_eq!(
        hour_series.groups[0].kind,
        MetricKind::Histogram(Temporality::Delta)
    );
    assert_eq!(hour_series.resolution, Resolution::Hour);
}

// Without the counters the writer sends about itself when it stops.
fn query_summary_rows(directory: &Path, table: &str) -> Vec<String> {
    let connection = Connection::open(directory.join("metrics-rollup.sqlite")).unwrap();
    let sql = format!(
        "SELECT json_array(series.name, summary.start_at, summary.count, summary.min,
                           summary.max, summary.sum, summary.last, summary.increase,
                           summary.seconds, summary.histogram)
         FROM {table} summary
         JOIN series ON series.id = summary.series_id
         WHERE series.name NOT LIKE 'otelo.%'
         ORDER BY series.name, summary.start_at"
    );
    connection
        .prepare(&sql)
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn rolling_a_minute_up_again_gives_the_same_rows() {
    let directory = tempfile::tempdir().unwrap();
    write_metrics(directory.path(), metrics_of_each_kind());
    let now = ten_tomorrow() + HOUR + 5 * MINUTE;
    roll_up_all_due(directory.path(), now);
    let (minute_rows, hour_rows) = (
        query_summary_rows(directory.path(), "minutes"),
        query_summary_rows(directory.path(), "hours"),
    );
    assert_eq!(minute_rows.len(), 9);
    assert_eq!(hour_rows.len(), 5);

    Connection::open(directory.path().join("metrics-rollup.sqlite"))
        .unwrap()
        .execute("DELETE FROM cursors", [])
        .unwrap();
    roll_up_all_due(directory.path(), now);
    assert_eq!(query_summary_rows(directory.path(), "minutes"), minute_rows);
    assert_eq!(query_summary_rows(directory.path(), "hours"), hour_rows);
}

#[test]
fn fills_the_hours_it_missed_while_the_daemon_was_down() {
    let directory = tempfile::tempdir().unwrap();
    write_metrics(
        directory.path(),
        vec![metric(
            "queue.lag",
            Points::Gauge(number_points(&[(5, 1.0), (3 * 3600 + 5, 7.0)])),
        )],
    );
    roll_up_all_due(directory.path(), ten_tomorrow() + 5 * MINUTE);
    assert_eq!(
        levels(&read_buckets(
            directory.path(),
            "queue.lag",
            Resolution::Minute
        )),
        [(1, 1.0, 1.0, 1.0, 1.0)]
    );

    // Four hours later the job rolls up an hour at a time until nothing is due.
    let passes = roll_up_all_due(directory.path(), ten_tomorrow() + 4 * HOUR + 5 * MINUTE);
    assert!(passes >= 4, "{passes} passes");
    assert_eq!(
        levels(&read_buckets(
            directory.path(),
            "queue.lag",
            Resolution::Minute
        )),
        [(1, 1.0, 1.0, 1.0, 1.0), (1, 7.0, 7.0, 7.0, 7.0)]
    );
    assert_eq!(
        levels(&read_buckets(
            directory.path(),
            "queue.lag",
            Resolution::Hour
        )),
        [(1, 1.0, 1.0, 1.0, 1.0), (1, 7.0, 7.0, 7.0, 7.0)]
    );
}

#[test]
fn a_step_of_summaries_is_a_whole_number_of_them() {
    let directory = tempfile::tempdir().unwrap();
    write_metrics(directory.path(), metrics_of_each_kind());
    roll_up_all_due(directory.path(), ten_tomorrow() + 10 * MINUTE);
    let metric_series = read_metric_series(
        directory.path(),
        "queue.lag",
        Resolution::Minute,
        90 * SECOND,
    );
    assert_eq!(metric_series.step_ns, 2 * MINUTE);
    assert_eq!(
        levels(&metric_series.groups[0].buckets),
        [(3, 1.0, 5.0, 3.0, 3.0)]
    );
}

#[test]
fn lists_the_series_that_have_summaries_in_the_range() {
    let directory = tempfile::tempdir().unwrap();
    write_metrics(directory.path(), metrics_of_each_kind());
    roll_up_all_due(directory.path(), ten_tomorrow() + 10 * MINUTE);
    let list_series_of_hour = |since: i64, query: &str| {
        let range = TimeRange::new(since, since + HOUR).unwrap();
        let reader = Reader::open(directory.path(), range).unwrap();
        let query = parse_query(query, Signal::Metrics).unwrap();
        let metric_list = reader.list_metrics(&query, Resolution::Minute, 10).unwrap();
        metric_list
            .series
            .into_iter()
            .map(|series| format!("{} {}", series.name, series.kind.name()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        list_series_of_hour(ten_tomorrow() + 30 * SECOND, ""),
        [
            "bytes.sent counter",
            "emails.sent counter",
            "query.duration histogram",
            "queue.lag gauge",
            "request.duration histogram",
        ]
    );
    assert_eq!(
        list_series_of_hour(ten_tomorrow(), "kind = gauge service = api"),
        ["queue.lag gauge"]
    );
    assert!(list_series_of_hour(ten_tomorrow() + 2 * HOUR, "").is_empty());
}

#[test]
fn deletes_the_summaries_past_their_retention_and_the_series_without_any() {
    let directory = tempfile::tempdir().unwrap();
    write_metrics(directory.path(), metrics_of_each_kind());
    roll_up_all_due(directory.path(), ten_tomorrow() + HOUR + 5 * MINUTE);
    let mut rollups = Rollups::open(directory.path()).unwrap();

    // The second minute and the hour are still kept.
    rollups
        .delete_summaries_before(ten_tomorrow() + MINUTE, ten_tomorrow())
        .unwrap();
    assert_eq!(query_summary_rows(directory.path(), "minutes").len(), 4);
    assert_eq!(query_summary_rows(directory.path(), "hours").len(), 5);

    rollups
        .delete_summaries_before(ten_tomorrow() + HOUR, ten_tomorrow() + HOUR)
        .unwrap();
    let connection = Connection::open(directory.path().join("metrics-rollup.sqlite")).unwrap();
    let remaining_row_count: i64 = connection
        .query_row(
            "SELECT (SELECT count(*) FROM series) + (SELECT count(*) FROM resources)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining_row_count, 0);
}
