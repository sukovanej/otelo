use std::collections::{BTreeMap, HashMap};

use anyhow::ensure;
use siner_storage::query::{
    Latency, MAX_BUCKETS, Operation, OperationDetail, RequestBucket, Requests, Service,
    ServiceBucket, ServiceStats, ServiceSummary, Services,
};
use siner_storage::{Attributes, Severity, SpanKind, SpanStatus};

use super::{WhereClause, new_statement_span, timestamp_from_nanos, truncate_to_limit};
use crate::Reader;

// Kind 2 is server and kind 5 is consumer.
const ENTRY_SPAN_CLAUSE: &str = "(s.parent_span_id IS NULL OR s.kind IN (2, 5))";

// A value is taken for the middle of its bucket, 2γ^i/(γ+1), which is off by (γ-1)/(γ+1), about 1%.
const SKETCH_BUCKET_GROWTH: f64 = 1.02;

// Buckets that grow by a factor keep a percentile's relative error at any size, in memory
// that grows with the logarithm of the range of the durations, not with their count.
#[derive(Default)]
struct DurationSketch {
    zeros: u64,
    counts: BTreeMap<i32, u64>,
}

impl DurationSketch {
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "the index of a bucket is below 2000 for any i64"
    )]
    fn add_duration(&mut self, nanos: i64) {
        if nanos <= 0 {
            self.zeros += 1;
        } else {
            let index = (nanos as f64).log(SKETCH_BUCKET_GROWTH).ceil() as i32;
            *self.counts.entry(index).or_default() += 1;
        }
    }

    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "an estimate"
    )]
    fn nearest_rank_quantile(&self, q: f64, count: u64) -> i64 {
        let rank = ((q * count as f64).ceil() as u64).max(1) - 1;
        let mut seen = self.zeros;
        if rank < seen {
            return 0;
        }
        for (&index, &n) in &self.counts {
            seen += n;
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
    pub(super) fn add_request(&mut self, duration_ns: i64, error: bool) {
        self.count += 1;
        self.errors += u64::from(error);
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
struct ServiceTally {
    requests: RequestTally,
    logs: u64,
    error_logs: u64,
    steps: HashMap<i64, (RequestTally, u64, u64)>,
}

impl ServiceTally {
    fn stats(&self) -> ServiceStats {
        ServiceStats {
            requests: self.requests.to_requests(),
            logs: self.logs,
            error_logs: self.error_logs,
        }
    }

    fn step_buckets(&self, first: i64, until: i64, step: i64) -> Vec<ServiceBucket> {
        let empty = (RequestTally::default(), 0, 0);
        (first..until)
            .step_by(usize::try_from(step).unwrap_or(usize::MAX))
            .map(|start| {
                let (requests, logs, error_logs) = self.steps.get(&start).unwrap_or(&empty);
                ServiceBucket {
                    start_at: timestamp_from_nanos(start),
                    requests: requests.to_requests(),
                    logs: *logs,
                    error_logs: *error_logs,
                }
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
enum TallyScope<'a> {
    Every,
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
            Self::Every => None,
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
    location: SpanLocation,
}

pub(super) type SpanLocation = (String, i64);

#[derive(Default)]
pub(super) struct OperationTally {
    pub(super) requests: RequestTally,
    pub(super) newest_request: Option<(i64, SpanLocation)>,
}

impl OperationTally {
    pub(super) fn add_request_at(
        &mut self,
        started_at: i64,
        duration_ns: i64,
        failed: bool,
        location: &SpanLocation,
    ) {
        self.requests.add_request(duration_ns, failed);
        if self
            .newest_request
            .as_ref()
            .is_none_or(|(newest_at, _)| started_at >= *newest_at)
        {
            self.newest_request = Some((started_at, location.clone()));
        }
    }

    pub(super) fn is_newest_request_at(&self, started_at: i64) -> bool {
        self.newest_request
            .as_ref()
            .is_some_and(|(newest_at, _)| *newest_at == started_at)
    }
}

impl Reader {
    pub(super) fn read_span_attributes<'a>(
        &self,
        rows: impl Iterator<Item = &'a SpanLocation>,
    ) -> anyhow::Result<HashMap<SpanLocation, Attributes>> {
        let mut by_day: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
        for (day, rowid) in rows {
            by_day.entry(day).or_default().push(*rowid);
        }
        let mut out = HashMap::new();
        for (day, rowids) in by_day {
            let list = rowids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let sql =
                format!("SELECT rowid, attributes FROM \"{day}\".spans WHERE rowid IN ({list})");
            let span = new_statement_span(&sql);
            let _entered = span.enter();
            let mut stmt = self.conn().prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut read = 0_i64;
            while let Some(row) = rows.next()? {
                read += 1;
                let attributes: String = row.get(1)?;
                out.insert(
                    (day.to_owned(), row.get(0)?),
                    serde_json::from_str(&attributes)?,
                );
            }
            span.record("db.response.returned_rows", read);
        }
        Ok(out)
    }

    pub(super) fn check_step(&self, step_ns: i64) -> anyhow::Result<()> {
        ensure!(step_ns > 0, "the step has to be longer than zero");
        ensure!(
            self.range().length() / step_ns <= MAX_BUCKETS,
            "the step makes more than {MAX_BUCKETS} buckets in the range; raise the step"
        );
        Ok(())
    }

    fn tally_services(
        &self,
        scope: TallyScope,
        step: i64,
        mut on_entry_span: impl FnMut(&EntrySpan),
    ) -> anyhow::Result<HashMap<String, ServiceTally>> {
        let mut tallies: HashMap<String, ServiceTally> = HashMap::new();

        let mut where_ = WhereClause::within_reader_range(self, "s.start_ts");
        where_.push_clause(ENTRY_SPAN_CLAUSE.into());
        if let Some(service) = scope.service() {
            where_.push_clause_with_param("r.service = :service", ":service", service.to_owned());
        }
        if let TallyScope::Operation { name, kind, .. } = scope {
            where_.push_clause_with_param("s.name = :name", ":name", name.to_owned());
            where_.push_clause_with_param("s.kind = :kind", ":kind", kind.number());
        }
        self.scan_rows(
            ["", ""],
            |day| {
                format!(
                    "SELECT r.service, s.name, s.kind, s.start_ts, s.duration_ns, s.status,
                            '{}', s.rowid
                     FROM {day}.spans s JOIN {day}.resources r ON r.id = s.resource_id
                     WHERE {}",
                    day.trim_matches('"'),
                    where_.sql_for_day(day)
                )
            },
            &where_,
            |row| {
                let entry = EntrySpan {
                    service: row.get(0)?,
                    name: row.get(1)?,
                    kind: SpanKind::from_number(row.get(2)?),
                    started_at: row.get(3)?,
                    duration_ns: row.get(4)?,
                    failed: SpanStatus::from_number(row.get(5)?).is_error(),
                    location: (row.get(6)?, row.get(7)?),
                };
                let tally = tallies.entry(entry.service.clone()).or_default();
                tally.requests.add_request(entry.duration_ns, entry.failed);
                let start = entry.started_at.div_euclid(step) * step;
                let (requests, _, _) = tally.steps.entry(start).or_default();
                requests.add_request(entry.duration_ns, entry.failed);
                on_entry_span(&entry);
                Ok(true)
            },
        )?;

        // No operation has logs.
        if matches!(scope, TallyScope::Operation { .. }) {
            return Ok(tallies);
        }
        let mut where_ = WhereClause::within_reader_range(self, "l.ts");
        where_.push_param(":step", step);
        if let Some(service) = scope.service() {
            where_.push_clause_with_param("r.service = :service", ":service", service.to_owned());
        }
        self.scan_rows(
            [
                "SELECT service, start, sum(n), sum(errors) FROM (",
                ") GROUP BY service, start",
            ],
            |day| {
                format!(
                    "SELECT r.service, l.ts / :step * :step AS start, count(*) AS n,
                            sum(l.severity >= {}) AS errors
                     FROM {day}.logs l JOIN {day}.resources r ON r.id = l.resource_id
                     WHERE {}
                     GROUP BY 1, 2",
                    Severity::ERROR.number(),
                    where_.sql_for_day(day)
                )
            },
            &where_,
            |row| {
                let logs = u64::try_from(row.get::<_, i64>(2)?)?;
                let errors = u64::try_from(row.get::<_, i64>(3)?)?;
                let tally = tallies.entry(row.get(0)?).or_default();
                tally.logs += logs;
                tally.error_logs += errors;
                let (_, step_logs, step_errors) = tally.steps.entry(row.get(1)?).or_default();
                *step_logs += logs;
                *step_errors += errors;
                Ok(true)
            },
        )?;
        Ok(tallies)
    }

    fn read_resource_attributes(
        &self,
        only_service: Option<&str>,
    ) -> anyhow::Result<HashMap<String, Attributes>> {
        let mut where_ = WhereClause::new();
        if let Some(service) = only_service {
            where_.push_clause_with_param("service = :service", ":service", service.to_owned());
        }
        let mut resources = HashMap::new();
        self.scan_rows(
            [
                "SELECT service, attributes FROM (",
                // A resource with attributes wins over one without, such as the writer's own.
                ") ORDER BY attributes != '{}', day, id",
            ],
            |day| {
                format!(
                    "SELECT '{}' AS day, id, service, attributes FROM {day}.resources WHERE {}",
                    day.trim_matches('"'),
                    where_.sql_for_day(day)
                )
            },
            &where_,
            |row| {
                let attributes: String = row.get(1)?;
                resources.insert(row.get(0)?, serde_json::from_str(&attributes)?);
                Ok(true)
            },
        )?;
        Ok(resources)
    }
}

pub(super) fn summarize_services(
    reader: &Reader,
    step_ns: i64,
    limit: usize,
) -> anyhow::Result<Services> {
    reader.check_step(step_ns)?;
    let tallies = reader.tally_services(TallyScope::Every, step_ns, |_| {})?;
    let mut resources = reader.read_resource_attributes(None)?;
    let first = reader.range().start_at().div_euclid(step_ns) * step_ns;
    let mut services: Vec<_> = tallies
        .into_iter()
        .map(|(service, tally)| ServiceSummary {
            resource: resources.remove(&service).unwrap_or_default(),
            stats: tally.stats(),
            buckets: tally.step_buckets(first, reader.range().end_at(), step_ns),
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
                &entry.location,
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
    let mut attributes = reader.read_span_attributes(
        operations
            .iter()
            .filter_map(|(_, o)| Some(&o.newest_request.as_ref()?.1)),
    )?;
    let operations = operations
        .into_iter()
        .map(|((name, kind), operation)| Operation {
            name,
            kind,
            attributes: operation
                .newest_request
                .and_then(|(_, row)| attributes.remove(&row))
                .unwrap_or_default(),
            requests: operation.requests.to_requests(),
        })
        .collect();
    let first = reader.range().start_at().div_euclid(step_ns) * step_ns;
    Ok(Service {
        service: service.to_owned(),
        resource: reader
            .read_resource_attributes(Some(service))?
            .remove(service)
            .unwrap_or_default(),
        start_at: timestamp_from_nanos(reader.range().start_at()),
        end_at: timestamp_from_nanos(reader.range().end_at()),
        step_ns,
        stats: tally.stats(),
        buckets: tally.step_buckets(first, reader.range().end_at(), step_ns),
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
    let mut newest: Option<(i64, SpanLocation)> = None;
    let scope = TallyScope::Operation {
        service,
        name,
        kind,
    };
    let mut tallies = reader.tally_services(scope, step_ns, |entry| {
        if newest
            .as_ref()
            .is_none_or(|(start, _)| entry.started_at >= *start)
        {
            newest = Some((entry.started_at, entry.location.clone()));
        }
    })?;
    let tally = tallies.remove(service).unwrap_or_default();
    let attributes = match newest {
        Some((_, row)) => reader
            .read_span_attributes(std::iter::once(&row))?
            .remove(&row),
        None => None,
    };
    let first = reader.range().start_at().div_euclid(step_ns) * step_ns;
    Ok(OperationDetail {
        service: service.to_owned(),
        name: name.to_owned(),
        kind,
        attributes: attributes.unwrap_or_default(),
        start_at: timestamp_from_nanos(reader.range().start_at()),
        end_at: timestamp_from_nanos(reader.range().end_at()),
        step_ns,
        requests: tally.requests.to_requests(),
        buckets: tally
            .step_buckets(first, reader.range().end_at(), step_ns)
            .into_iter()
            .map(|bucket| RequestBucket {
                start_at: bucket.start_at,
                requests: bucket.requests,
            })
            .collect(),
    })
}
