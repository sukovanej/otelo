use std::collections::{BTreeMap, HashMap};
use std::ops::ControlFlow;

use anyhow::ensure;
use otelo_indexed_storage::query::{
    Latency, MAX_BUCKETS_IN_RANGE, Operation, OperationDetail, RequestBucket, Requests, Service,
    ServiceBucket, ServiceStats, ServiceSummary, Services,
};
use otelo_indexed_storage::{Attributes, Severity, SpanKind, SpanStatus};

use super::{WhereClause, timestamp_from_nanos, truncate_to_limit};
use crate::Reader;

fn entry_span_condition() -> String {
    format!(
        "(span.parent_span_id IS NULL OR span.kind IN ({}, {}))",
        SpanKind::Server.number(),
        SpanKind::Consumer.number()
    )
}

// A value is taken for the middle of its bucket, 2γ^i/(γ+1), which is off by (γ-1)/(γ+1), about 1%.
const SKETCH_BUCKET_GROWTH: f64 = 1.02;

// Buckets that grow by a factor keep the relative error of a percentile, in memory that grows with the logarithm of the durations.
#[derive(Default)]
struct DurationSketch {
    zero_count: u64,
    counts_by_bucket_index: BTreeMap<i32, u64>,
}

impl DurationSketch {
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "the index of a bucket is below 2000 for any i64"
    )]
    fn add_duration(&mut self, duration_ns: i64) {
        if duration_ns <= 0 {
            self.zero_count += 1;
        } else {
            let index = (duration_ns as f64).log(SKETCH_BUCKET_GROWTH).ceil() as i32;
            *self.counts_by_bucket_index.entry(index).or_default() += 1;
        }
    }

    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "an estimate"
    )]
    fn nearest_rank_quantile(&self, quantile: f64, count: u64) -> i64 {
        let rank = ((quantile * count as f64).ceil() as u64).max(1) - 1;
        let mut seen = self.zero_count;
        if rank < seen {
            return 0;
        }
        for (&index, &bucket_count) in &self.counts_by_bucket_index {
            seen += bucket_count;
            if rank < seen {
                return (2.0 * SKETCH_BUCKET_GROWTH.powi(index) / (SKETCH_BUCKET_GROWTH + 1.0))
                    .round() as i64;
            }
        }
        0
    }
}

#[derive(Default)]
pub(super) struct RequestTally {
    count: u64,
    errors: u64,
    pub(super) total_ns: i64,
    sketch: DurationSketch,
}

impl RequestTally {
    pub(super) fn add_request(&mut self, duration_ns: i64, failed: bool) {
        self.count += 1;
        self.errors += u64::from(failed);
        self.total_ns = self.total_ns.saturating_add(duration_ns);
        self.sketch.add_duration(duration_ns);
    }

    pub(super) fn to_requests(&self) -> Requests {
        Requests {
            count: self.count,
            errors: self.errors,
            total_ns: self.total_ns,
            latency: (self.count > 0).then(|| Latency {
                p50: self.sketch.nearest_rank_quantile(0.5, self.count),
                p95: self.sketch.nearest_rank_quantile(0.95, self.count),
                p99: self.sketch.nearest_rank_quantile(0.99, self.count),
            }),
        }
    }
}

#[derive(Default)]
struct StepTally {
    requests: RequestTally,
    logs: u64,
    error_logs: u64,
}

#[derive(Default)]
struct ServiceTally {
    total: StepTally,
    steps: HashMap<i64, StepTally>,
}

impl ServiceTally {
    fn to_service_stats(&self) -> ServiceStats {
        ServiceStats {
            requests: self.total.requests.to_requests(),
            logs: self.total.logs,
            error_logs: self.total.error_logs,
        }
    }

    fn to_service_buckets(
        &self,
        first_step_at: i64,
        end_at: i64,
        step_ns: i64,
    ) -> Vec<ServiceBucket> {
        let empty = StepTally::default();
        (first_step_at..end_at)
            .step_by(usize::try_from(step_ns).unwrap_or(usize::MAX))
            .map(|start_at| {
                let step = self.steps.get(&start_at).unwrap_or(&empty);
                ServiceBucket {
                    start_at: timestamp_from_nanos(start_at),
                    requests: step.requests.to_requests(),
                    logs: step.logs,
                    error_logs: step.error_logs,
                }
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
enum TallyScope<'a> {
    EveryService,
    Service(&'a str),
    Operation {
        service: &'a str,
        name: &'a str,
        kind: SpanKind,
    },
}

impl<'a> TallyScope<'a> {
    const fn service(self) -> Option<&'a str> {
        match self {
            Self::EveryService => None,
            Self::Service(service) | Self::Operation { service, .. } => Some(service),
        }
    }
}

struct EntrySpan {
    service: String,
    name: String,
    kind: SpanKind,
    started_at: i64,
    duration_ns: i64,
    failed: bool,
    rowid: SpanRowid,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct SpanRowid(pub(super) i64);

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct NewestRequest<Detail = ()> {
    pub(super) started_at: i64,
    pub(super) rowid: SpanRowid,
    pub(super) detail: Detail,
}

fn keep_newest_request<Detail>(
    newest_request: &mut Option<NewestRequest<Detail>>,
    started_at: i64,
    rowid: SpanRowid,
    detail: impl FnOnce() -> Detail,
) {
    if newest_request
        .as_ref()
        .is_none_or(|request| started_at >= request.started_at)
    {
        *newest_request = Some(NewestRequest {
            started_at,
            rowid,
            detail: detail(),
        });
    }
}

pub(super) struct OperationTally<Detail = ()> {
    pub(super) requests: RequestTally,
    pub(super) newest_request: Option<NewestRequest<Detail>>,
}

impl<Detail> Default for OperationTally<Detail> {
    fn default() -> Self {
        Self {
            requests: RequestTally::default(),
            newest_request: None,
        }
    }
}

impl<Detail> OperationTally<Detail> {
    pub(super) fn add_request_at(
        &mut self,
        started_at: i64,
        duration_ns: i64,
        failed: bool,
        rowid: SpanRowid,
        detail: impl FnOnce() -> Detail,
    ) {
        self.requests.add_request(duration_ns, failed);
        keep_newest_request(&mut self.newest_request, started_at, rowid, detail);
    }
}

impl Reader {
    pub(super) fn read_span_attributes(
        &self,
        rowids: impl Iterator<Item = SpanRowid>,
    ) -> anyhow::Result<HashMap<SpanRowid, Attributes>> {
        let rowid_list = rowids
            .map(|rowid| rowid.0.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        if rowid_list.is_empty() {
            return Ok(HashMap::new());
        }
        let sql = format!("SELECT rowid, attributes FROM spans WHERE rowid IN ({rowid_list})");
        let mut attributes_by_rowid = HashMap::new();
        self.scan_rows(&sql, &WhereClause::new(), |row| {
            let attributes: String = row.get(1)?;
            attributes_by_rowid.insert(SpanRowid(row.get(0)?), serde_json::from_str(&attributes)?);
            Ok(ControlFlow::Continue(()))
        })?;
        Ok(attributes_by_rowid)
    }

    pub(super) fn check_step(&self, step_ns: i64) -> anyhow::Result<()> {
        ensure!(step_ns > 0, "the step has to be longer than zero");
        ensure!(
            self.range().length_ns() / step_ns <= MAX_BUCKETS_IN_RANGE,
            "the step makes more than {MAX_BUCKETS_IN_RANGE} buckets in the range; raise the step"
        );
        Ok(())
    }

    fn tally_services(
        &self,
        scope: TallyScope,
        step_ns: i64,
        mut on_entry_span: impl FnMut(&EntrySpan),
    ) -> anyhow::Result<HashMap<String, ServiceTally>> {
        let mut tallies: HashMap<String, ServiceTally> = HashMap::new();

        let mut where_clause = WhereClause::within_reader_range(self, "span.started_at");
        where_clause.push_condition(entry_span_condition());
        if let Some(service) = scope.service() {
            where_clause.push_condition_with_param(
                "resource.service = :service",
                ":service",
                service.to_owned(),
            );
        }
        if let TallyScope::Operation { name, kind, .. } = scope {
            where_clause.push_condition_with_param("span.name = :name", ":name", name.to_owned());
            where_clause.push_condition_with_param("span.kind = :kind", ":kind", kind.number());
        }
        let sql = format!(
            "SELECT resource.service, span.name, span.kind, span.started_at, span.duration_ns,
                    span.status_code, span.rowid
             FROM spans span
             JOIN resources resource ON resource.id = span.resource_id
             WHERE {}",
            where_clause.sql()
        );
        self.scan_rows(&sql, &where_clause, |row| {
            let entry = EntrySpan {
                service: row.get(0)?,
                name: row.get(1)?,
                kind: SpanKind::from_number(row.get(2)?),
                started_at: row.get(3)?,
                duration_ns: row.get(4)?,
                failed: SpanStatus::from_number(row.get(5)?).is_error(),
                rowid: SpanRowid(row.get(6)?),
            };
            let tally = tallies.entry(entry.service.clone()).or_default();
            tally
                .total
                .requests
                .add_request(entry.duration_ns, entry.failed);
            let step_at = entry.started_at.div_euclid(step_ns) * step_ns;
            tally
                .steps
                .entry(step_at)
                .or_default()
                .requests
                .add_request(entry.duration_ns, entry.failed);
            on_entry_span(&entry);
            Ok(ControlFlow::Continue(()))
        })?;

        // No operation has logs.
        if !matches!(scope, TallyScope::Operation { .. }) {
            self.tally_logs(scope.service(), step_ns, &mut tallies)?;
        }
        Ok(tallies)
    }

    fn tally_logs(
        &self,
        only_service: Option<&str>,
        step_ns: i64,
        tallies: &mut HashMap<String, ServiceTally>,
    ) -> anyhow::Result<()> {
        let mut where_clause = WhereClause::within_reader_range(self, "log.logged_at");
        where_clause.push_param(":step", step_ns);
        if let Some(service) = only_service {
            where_clause.push_condition_with_param(
                "resource.service = :service",
                ":service",
                service.to_owned(),
            );
        }
        let sql = format!(
            "SELECT resource.service, log.logged_at / :step * :step AS step_start_at, count(*),
                    sum(log.severity_number >= {})
             FROM logs log
             JOIN resources resource ON resource.id = log.resource_id
             WHERE {}
             GROUP BY resource.service, step_start_at",
            Severity::ERROR.number(),
            where_clause.sql()
        );
        self.scan_rows(&sql, &where_clause, |row| {
            let logs = u64::try_from(row.get::<_, i64>(2)?)?;
            let error_logs = u64::try_from(row.get::<_, i64>(3)?)?;
            let tally = tallies.entry(row.get(0)?).or_default();
            tally.total.logs += logs;
            tally.total.error_logs += error_logs;
            let step = tally.steps.entry(row.get(1)?).or_default();
            step.logs += logs;
            step.error_logs += error_logs;
            Ok(ControlFlow::Continue(()))
        })?;
        Ok(())
    }

    fn read_resource_attributes(
        &self,
        only_service: Option<&str>,
    ) -> anyhow::Result<HashMap<String, Attributes>> {
        let mut where_clause = WhereClause::new();
        if let Some(service) = only_service {
            where_clause.push_condition_with_param(
                "service = :service",
                ":service",
                service.to_owned(),
            );
        }
        where_clause.push_param(":no_attributes", Attributes::new().to_json());
        let mut resources = HashMap::new();
        // A resource with attributes wins over one without, and a newer one over an older one.
        let sql = format!(
            "SELECT service, attributes, attributes != :no_attributes AS has_attributes
             FROM resources
             WHERE {}
             ORDER BY has_attributes, id",
            where_clause.sql()
        );
        self.scan_rows(&sql, &where_clause, |row| {
            let attributes: String = row.get(1)?;
            resources.insert(row.get(0)?, serde_json::from_str(&attributes)?);
            Ok(ControlFlow::Continue(()))
        })?;
        Ok(resources)
    }
}

pub(super) fn summarize_services(
    reader: &Reader,
    step_ns: i64,
    limit: usize,
) -> anyhow::Result<Services> {
    reader.check_step(step_ns)?;
    let tallies = reader.tally_services(TallyScope::EveryService, step_ns, |_| {})?;
    let mut resources = reader.read_resource_attributes(None)?;
    let first_step_at = reader.range().start_at().div_euclid(step_ns) * step_ns;
    let mut services: Vec<_> = tallies
        .into_iter()
        .map(|(service, tally)| ServiceSummary {
            resource: resources.remove(&service).unwrap_or_default(),
            stats: tally.to_service_stats(),
            buckets: tally.to_service_buckets(first_step_at, reader.range().end_at(), step_ns),
            service,
        })
        .collect();
    services.sort_by(|a, b| {
        (b.stats.requests.count, b.stats.logs)
            .cmp(&(a.stats.requests.count, a.stats.logs))
            .then_with(|| a.service.cmp(&b.service))
    });
    let truncated = truncate_to_limit(&mut services, limit);
    Ok(Services {
        start_at: timestamp_from_nanos(reader.range().start_at()),
        end_at: timestamp_from_nanos(reader.range().end_at()),
        step_ns,
        services,
        truncated,
    })
}

pub(super) fn summarize_service(
    reader: &Reader,
    service: &str,
    step_ns: i64,
    limit: usize,
) -> anyhow::Result<Service> {
    reader.check_step(step_ns)?;
    let mut operations: HashMap<(String, SpanKind), OperationTally> = HashMap::new();
    let mut tallies = reader.tally_services(TallyScope::Service(service), step_ns, |entry| {
        operations
            .entry((entry.name.clone(), entry.kind))
            .or_default()
            .add_request_at(
                entry.started_at,
                entry.duration_ns,
                entry.failed,
                entry.rowid,
                || (),
            );
    })?;
    let tally = tallies.remove(service).unwrap_or_default();
    let mut operations: Vec<_> = operations.into_iter().collect();
    operations.sort_by(|((a_name, a_kind), a), ((b_name, b_kind), b)| {
        b.requests
            .count
            .cmp(&a.requests.count)
            .then_with(|| a_name.cmp(b_name))
            .then_with(|| a_kind.cmp(b_kind))
    });
    let truncated = truncate_to_limit(&mut operations, limit);
    let mut attributes =
        reader.read_span_attributes(operations.iter().filter_map(|(_, operation)| {
            operation
                .newest_request
                .as_ref()
                .map(|newest_request| newest_request.rowid)
        }))?;
    let operations = operations
        .into_iter()
        .map(|((name, kind), operation)| Operation {
            name,
            kind,
            attributes: operation
                .newest_request
                .and_then(|newest_request| attributes.remove(&newest_request.rowid))
                .unwrap_or_default(),
            requests: operation.requests.to_requests(),
        })
        .collect();
    let first_step_at = reader.range().start_at().div_euclid(step_ns) * step_ns;
    Ok(Service {
        service: service.to_owned(),
        resource: reader
            .read_resource_attributes(Some(service))?
            .remove(service)
            .unwrap_or_default(),
        start_at: timestamp_from_nanos(reader.range().start_at()),
        end_at: timestamp_from_nanos(reader.range().end_at()),
        step_ns,
        stats: tally.to_service_stats(),
        buckets: tally.to_service_buckets(first_step_at, reader.range().end_at(), step_ns),
        operations,
        truncated,
    })
}

pub(super) fn summarize_operation(
    reader: &Reader,
    service: &str,
    name: &str,
    kind: SpanKind,
    step_ns: i64,
) -> anyhow::Result<OperationDetail> {
    reader.check_step(step_ns)?;
    let mut newest_request: Option<NewestRequest> = None;
    let scope = TallyScope::Operation {
        service,
        name,
        kind,
    };
    let mut tallies = reader.tally_services(scope, step_ns, |entry| {
        keep_newest_request(&mut newest_request, entry.started_at, entry.rowid, || ());
    })?;
    let tally = tallies.remove(service).unwrap_or_default();
    let attributes = match newest_request {
        Some(newest_request) => reader
            .read_span_attributes(std::iter::once(newest_request.rowid))?
            .remove(&newest_request.rowid),
        None => None,
    };
    let first_step_at = reader.range().start_at().div_euclid(step_ns) * step_ns;
    Ok(OperationDetail {
        service: service.to_owned(),
        name: name.to_owned(),
        kind,
        attributes: attributes.unwrap_or_default(),
        start_at: timestamp_from_nanos(reader.range().start_at()),
        end_at: timestamp_from_nanos(reader.range().end_at()),
        step_ns,
        requests: tally.total.requests.to_requests(),
        buckets: tally
            .to_service_buckets(first_step_at, reader.range().end_at(), step_ns)
            .into_iter()
            .map(|bucket| RequestBucket {
                start_at: bucket.start_at,
                requests: bucket.requests,
            })
            .collect(),
    })
}
