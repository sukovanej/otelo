mod common;

use otelo_indexed_storage::query::{LogCounts, LogGroupingField, RankOrder};
use otelo_indexed_storage::{
    AttributeValue, Attributes, Log, RangeQueries, Records, Resource, Severity, TimeRange,
    TraceContext,
};
use otelo_indexed_storage_sqlite::{Config, Day, Reader};
use otelo_query::{Signal, parse_query};
use serde_json::{Value, json};

const SECOND: i64 = 1_000_000_000;
const MINUTE: i64 = 60 * SECOND;

fn log(logged_at: i64, severity: Severity, attributes: &Value) -> Log {
    Log {
        logged_at,
        severity_number: severity,
        body: "a line".into(),
        trace_context: TraceContext::None,
        attributes: serde_json::from_value(attributes.clone()).unwrap(),
    }
}

fn records(service: &str, logs: Vec<Log>) -> Records {
    Records {
        resource: Resource {
            service: service.into(),
            attributes: Attributes::new(),
        },
        logs,
        spans: Vec::new(),
        metrics: Vec::new(),
    }
}

struct Fixture {
    directory: tempfile::TempDir,
    today_start_at: i64,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let today_start_at = Day::today().start_at();
        let api_logs = vec![
            log(
                today_start_at + SECOND,
                Severity::INFO,
                &json!({"user.id": 7}),
            ),
            log(
                today_start_at + 2 * SECOND,
                Severity::from_number(10),
                &json!({"user.id": 7}),
            ),
            log(
                today_start_at + MINUTE,
                Severity::ERROR,
                &json!({"user.id": 8}),
            ),
        ];
        let worker_logs = vec![log(
            today_start_at + MINUTE + SECOND,
            Severity::WARN,
            &json!({}),
        )];
        common::index_batches(
            Config::new(directory.path().to_owned(), common::INDEX_RETENTION_DAYS),
            vec![vec![
                records("api", api_logs),
                records("worker", worker_logs),
            ]],
        );
        Self {
            directory,
            today_start_at,
        }
    }

    fn count_logs(&self, query: &str, by: &[&str], limit: usize) -> LogCounts {
        self.count_logs_in_order(query, by, RankOrder::Highest, limit)
    }

    fn count_logs_in_order(
        &self,
        query: &str,
        by: &[&str],
        order: RankOrder,
        limit: usize,
    ) -> LogCounts {
        let by: Vec<LogGroupingField> = by.iter().map(|field| field.parse().unwrap()).collect();
        Reader::open(
            self.directory.path(),
            TimeRange::new(self.today_start_at, self.today_start_at + 2 * MINUTE).unwrap(),
        )
        .unwrap()
        .count_logs(
            &parse_query(query, Signal::Logs).unwrap(),
            &by,
            order,
            MINUTE,
            limit,
        )
        .unwrap()
    }
}

type GroupSummary<'a> = (Vec<(&'a str, &'a AttributeValue)>, u64, Vec<u64>);

fn summarize_groups(counts: &LogCounts) -> Vec<GroupSummary<'_>> {
    counts
        .groups
        .iter()
        .map(|group| {
            (
                group
                    .values
                    .iter()
                    .map(|(field, value)| (field.as_str(), value))
                    .collect(),
                group.count,
                group.buckets.iter().map(|bucket| bucket.count).collect(),
            )
        })
        .collect()
}

#[test]
fn lines_count_by_step_and_group_the_most_lines_first() {
    let fixture = Fixture::new();
    let by_service = fixture.count_logs("", &["service"], 10);
    assert_eq!(by_service.count, 4);
    let steps: Vec<_> = by_service
        .buckets
        .iter()
        .map(|bucket| bucket.count)
        .collect();
    assert_eq!(steps, [2, 2]);
    let api = AttributeValue::from("api");
    let worker = AttributeValue::from("worker");
    assert_eq!(
        summarize_groups(&by_service),
        [
            (vec![("service", &api)], 3, vec![2, 1]),
            (vec![("service", &worker)], 1, vec![0, 1]),
        ]
    );
    assert!(!by_service.truncated);
}

#[test]
fn a_level_groups_every_severity_number_of_it() {
    let fixture = Fixture::new();
    let by_level = fixture.count_logs("", &["level"], 10);
    let info = AttributeValue::from("info");
    let warn = AttributeValue::from("warn");
    let error = AttributeValue::from("error");
    assert_eq!(
        summarize_groups(&by_level),
        [
            (vec![("level", &info)], 2, vec![2, 0]),
            (vec![("level", &error)], 1, vec![0, 1]),
            (vec![("level", &warn)], 1, vec![0, 1]),
        ]
    );
}

#[test]
fn the_limit_keeps_the_groups_with_the_most_lines() {
    let fixture = Fixture::new();
    let by_user = fixture.count_logs(r#"service = "api""#, &["user.id"], 1);
    assert!(by_user.truncated);
    assert_eq!(by_user.count, 3);
    assert_eq!(
        summarize_groups(&by_user),
        [(vec![("user.id", &AttributeValue::Int(7))], 2, vec![2, 0])]
    );

    let without_grouping = fixture.count_logs("level >= warn", &[], 10);
    assert_eq!(
        summarize_groups(&without_grouping),
        [(vec![], 2, vec![0, 2])]
    );
    assert!(
        fixture
            .count_logs(r#"service = "nobody""#, &[], 10)
            .groups
            .is_empty()
    );
}

#[test]
fn logs_group_only_by_names_a_line_has() {
    assert!("body".parse::<LogGroupingField>().is_err());
    assert_eq!("level".parse(), Ok(LogGroupingField::Level));
    assert_eq!(
        "resource.host.name".parse(),
        Ok(LogGroupingField::Resource("host.name".into()))
    );
}

#[test]
fn lines_without_the_grouped_attribute_make_a_group_of_their_own() {
    let fixture = Fixture::new();
    let by_user = fixture.count_logs("", &["user.id"], 10);
    let seven = AttributeValue::Int(7);
    let eight = AttributeValue::Int(8);
    assert_eq!(
        summarize_groups(&by_user),
        [
            (vec![("user.id", &seven)], 2, vec![2, 0]),
            (vec![], 1, vec![0, 1]),
            (vec![("user.id", &eight)], 1, vec![0, 1]),
        ]
    );
}

#[test]
fn the_lowest_order_keeps_the_groups_with_the_fewest_lines() {
    let fixture = Fixture::new();
    let by_service = fixture.count_logs_in_order("", &["service"], RankOrder::Lowest, 1);
    assert!(by_service.truncated);
    assert_eq!(
        summarize_groups(&by_service),
        [(
            vec![("service", &AttributeValue::from("worker"))],
            1,
            vec![0, 1]
        )]
    );
}
