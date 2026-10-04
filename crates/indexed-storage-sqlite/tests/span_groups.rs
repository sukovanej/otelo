mod common;

use otelo_indexed_storage::query::{
    GroupBuckets, RankOrder, SpanGroup, SpanGroupRank, SpanGroupRanking, SpanGroupingField,
    SpanGroups,
};
use otelo_indexed_storage::{
    AttributeValue, Attributes, RangeQueries, Records, Resource, Span, SpanId, SpanKind,
    SpanStatus, TimeRange, TraceId,
};
use otelo_indexed_storage_sqlite::{Config, Day, Reader};
use otelo_query::{Signal, parse_query};
use serde_json::{Value, json};

const SECOND: i64 = 1_000_000_000;
const MILLISECOND: i64 = 1_000_000;
const MINUTE: i64 = 60 * SECOND;

fn attributes_from_json(value: &Value) -> Attributes {
    serde_json::from_value(value.clone()).unwrap()
}

fn span(
    span_id_byte: u8,
    kind: SpanKind,
    name: &str,
    started_at: i64,
    duration_ms: i64,
    attributes: &Value,
) -> Span {
    Span {
        trace_id: TraceId([span_id_byte; 16]),
        span_id: SpanId([span_id_byte; 8]),
        parent_span_id: None,
        name: name.into(),
        kind,
        started_at,
        duration_ns: duration_ms * MILLISECOND,
        status_code: SpanStatus::Unset,
        attributes: attributes_from_json(attributes),
        events: Vec::new(),
    }
}

fn records(service: &str, resource: &Value, spans: Vec<Span>) -> Records {
    Records {
        resource: Resource {
            service: service.into(),
            attributes: attributes_from_json(resource),
        },
        logs: Vec::new(),
        spans,
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
        let get_users = json!({"http.request.method": "GET", "http.route": "/users"});
        let api_spans = vec![
            Span {
                status_code: SpanStatus::Error,
                ..span(
                    1,
                    SpanKind::Server,
                    "GET /users",
                    today_start_at + SECOND,
                    30,
                    &json!({"http.request.method": "GET", "http.route": "/users", "first": true}),
                )
            },
            span(
                2,
                SpanKind::Server,
                "GET /users",
                today_start_at + MINUTE,
                10,
                &get_users,
            ),
            span(
                3,
                SpanKind::Server,
                "POST /orders",
                today_start_at + MINUTE,
                5,
                &json!({"http.request.method": "POST", "http.route": "/orders"}),
            ),
            span(
                4,
                SpanKind::Server,
                "GET",
                today_start_at + MINUTE,
                1,
                &json!({"http.request.method": "GET", "http.response.status_code": 404, "cached": true}),
            ),
            span(
                5,
                SpanKind::Client,
                "SELECT",
                today_start_at + MINUTE,
                2,
                &json!({"db.system.name": "sqlite", "db.query.text": "SELECT 1"}),
            ),
        ];
        let worker_spans = vec![span(
            6,
            SpanKind::Server,
            "GET /users",
            today_start_at + MINUTE,
            100,
            &get_users,
        )];
        common::index_batches(
            Config::new(directory.path().to_owned()),
            vec![vec![
                records("api", &json!({"host.name": "a"}), api_spans),
                records("worker", &json!({"host.name": "b"}), worker_spans),
            ]],
        );
        Self {
            directory,
            today_start_at,
        }
    }

    fn group_spans(&self, query: &str, by: &[&str], limit: usize) -> SpanGroups {
        self.group_spans_with_buckets(query, by, GroupBuckets::Omitted, limit)
    }

    fn group_spans_with_buckets(
        &self,
        query: &str,
        by: &[&str],
        group_buckets: GroupBuckets,
        limit: usize,
    ) -> SpanGroups {
        self.rank_span_groups(query, by, SpanGroupRanking::default(), group_buckets, limit)
    }

    fn rank_span_groups(
        &self,
        query: &str,
        by: &[&str],
        ranking: SpanGroupRanking,
        group_buckets: GroupBuckets,
        limit: usize,
    ) -> SpanGroups {
        let by: Vec<SpanGroupingField> = by.iter().map(|field| field.parse().unwrap()).collect();
        Reader::open(
            self.directory.path(),
            TimeRange::new(self.today_start_at, self.today_start_at + 2 * MINUTE).unwrap(),
        )
        .unwrap()
        .list_span_groups(
            &parse_query(query, Signal::Spans).unwrap(),
            &by,
            ranking,
            MINUTE,
            group_buckets,
            limit,
        )
        .unwrap()
    }
}

fn summarize_group(group: &SpanGroup) -> (Vec<(&str, &AttributeValue)>, u64, u64) {
    let values = group
        .values
        .iter()
        .map(|(field, value)| (field.as_str(), value))
        .collect();
    (values, group.spans.count, group.spans.errors)
}

#[test]
fn spans_group_by_attributes_the_most_time_first() {
    let fixture = Fixture::new();
    let routes = fixture.group_spans(
        r#"service = "api" kind = server has(http.request.method)"#,
        &["http.request.method", "http.route"],
        10,
    );
    let get = AttributeValue::from("GET");
    let post = AttributeValue::from("POST");
    let users = AttributeValue::from("/users");
    let orders = AttributeValue::from("/orders");
    let groups: Vec<_> = routes.groups.iter().map(summarize_group).collect();
    assert_eq!(
        groups,
        [
            (
                vec![("http.request.method", &get), ("http.route", &users)],
                2,
                1
            ),
            (
                vec![("http.request.method", &post), ("http.route", &orders)],
                1,
                0
            ),
            (vec![("http.request.method", &get)], 1, 0),
        ]
    );
    assert!(!routes.truncated);

    let users_group = &routes.groups[0];
    assert_eq!(users_group.name, "GET /users");
    assert!(users_group.attributes.get("first").is_none());
    assert_eq!(users_group.spans.total_ns, 40 * MILLISECOND);

    assert_eq!((routes.spans.count, routes.spans.errors), (4, 1));
    let counts: Vec<_> = routes
        .buckets
        .iter()
        .map(|bucket| bucket.spans.count)
        .collect();
    assert_eq!(counts, [1, 3]);
}

#[test]
fn the_limit_keeps_the_groups_with_the_most_time() {
    let fixture = Fixture::new();
    let routes = fixture.group_spans("kind = server", &["http.route"], 1);
    assert!(routes.truncated);
    assert_eq!(routes.groups.len(), 1);
    assert_eq!(routes.groups[0].values["http.route"], "/users");
    assert_eq!(routes.groups[0].spans.count, 3);
    assert_eq!(routes.spans.count, 5);
}

#[test]
fn spans_group_by_service_name_and_resource() {
    let fixture = Fixture::new();
    let by_service = fixture.group_spans(
        r#"name = "GET /users""#,
        &["service", "name", "resource.host.name"],
        10,
    );
    let summaries: Vec<_> = by_service
        .groups
        .iter()
        .map(|group| {
            (
                group.values["service"].clone(),
                group.values["name"].clone(),
                group.values["resource.host.name"].clone(),
            )
        })
        .collect();
    assert_eq!(
        summaries,
        [
            ("worker".into(), "GET /users".into(), "b".into()),
            ("api".into(), "GET /users".into(), "a".into()),
        ]
    );
}

#[test]
fn spans_without_grouping_fields_are_one_group() {
    let fixture = Fixture::new();
    let every_span = fixture.group_spans("", &[], 10);
    assert_eq!(every_span.groups.len(), 1);
    assert!(every_span.groups[0].values.is_empty());
    assert_eq!(every_span.groups[0].spans.count, 6);

    let numbers = fixture.group_spans(
        "has(http.response.status_code)",
        &["http.response.status_code"],
        10,
    );
    assert_eq!(
        numbers.groups[0].values["http.response.status_code"],
        AttributeValue::Int(404)
    );

    let flags = fixture.group_spans("has(cached)", &["cached"], 10);
    assert_eq!(flags.groups[0].values["cached"], AttributeValue::Bool(true));

    let nothing = fixture.group_spans(r#"service = "nobody""#, &["http.route"], 10);
    assert!(nothing.groups.is_empty());
    assert_eq!(nothing.spans.count, 0);
    assert_eq!(nothing.buckets.len(), 2);
}

#[test]
fn spans_group_only_by_names_a_span_has() {
    assert!("duration".parse::<SpanGroupingField>().is_err());
    assert_eq!(
        "attr.service".parse::<SpanGroupingField>(),
        Ok(SpanGroupingField::Attribute("service".into()))
    );
    assert_eq!(
        SpanGroupingField::Attribute("service".into()).to_string(),
        "attr.service"
    );
}

#[test]
fn each_group_counts_its_spans_by_step_when_asked() {
    let fixture = Fixture::new();
    let without = fixture.group_spans("kind = server", &["service"], 10);
    assert!(without.groups.iter().all(|group| group.buckets.is_none()));

    let by_service =
        fixture.group_spans_with_buckets("kind = server", &["service"], GroupBuckets::Counted, 10);
    let steps: Vec<_> = by_service
        .groups
        .iter()
        .map(|group| {
            let steps: Vec<_> = group
                .buckets
                .as_ref()
                .unwrap()
                .iter()
                .map(|bucket| (bucket.spans.count, bucket.spans.errors))
                .collect();
            (group.values["service"].clone(), steps)
        })
        .collect();
    assert_eq!(
        steps,
        [
            (AttributeValue::from("worker"), vec![(0, 0), (1, 0)]),
            (AttributeValue::from("api"), vec![(1, 1), (3, 0)]),
        ]
    );
}

#[test]
fn the_kept_groups_count_their_steps_when_the_limit_drops_others() {
    let fixture = Fixture::new();
    let by_service =
        fixture.group_spans_with_buckets("kind = server", &["service"], GroupBuckets::Counted, 1);
    assert!(by_service.truncated);
    assert_eq!(by_service.groups.len(), 1);
    let group = &by_service.groups[0];
    let counted_by_step: u64 = group
        .buckets
        .as_ref()
        .unwrap()
        .iter()
        .map(|bucket| bucket.spans.count)
        .sum();
    assert_eq!(counted_by_step, group.spans.count);
}

#[test]
fn groups_rank_by_any_number_of_their_spans_from_either_end() {
    let fixture = Fixture::new();
    let routes_ranked_by = |rank, order, limit| {
        let ranking = SpanGroupRanking { rank, order };
        let groups = fixture.rank_span_groups(
            "kind = server",
            &["http.route"],
            ranking,
            GroupBuckets::Omitted,
            limit,
        );
        let routes: Vec<_> = groups
            .groups
            .iter()
            .map(|group| group.values.get("http.route").cloned())
            .collect();
        (routes, groups.truncated)
    };
    let users = Some(AttributeValue::from("/users"));
    let orders = Some(AttributeValue::from("/orders"));
    assert_eq!(
        routes_ranked_by(SpanGroupRank::P95, RankOrder::Lowest, 2),
        (vec![None, orders.clone()], true)
    );
    assert_eq!(
        routes_ranked_by(SpanGroupRank::P95, RankOrder::Highest, 10),
        (vec![users.clone(), orders, None], false)
    );
    assert_eq!(
        routes_ranked_by(SpanGroupRank::ErrorRate, RankOrder::Highest, 1).0,
        [users]
    );
}
