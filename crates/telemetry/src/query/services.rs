use std::collections::{BTreeMap, HashMap};

use anyhow::ensure;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::metrics::MAX_BUCKETS;
use super::{Filter, STATUS_ERROR, cut, statement_span, time};
use crate::{Attributes, Reader};

/// The spans that enter a service, which count as its requests: a root span,
/// or a span of the server or the consumer kind.
const ENTRY: &str = "(s.parent_span_id IS NULL OR s.kind IN (2, 5))";

/// The lowest severity number of an error log.
const ERROR_SEVERITY: i32 = 17;

/// The services that sent spans or logs in a range, the busiest first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Services {
    /// The range, after the retention capped it.
    #[schema(value_type = String, format = DateTime)]
    pub since: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub until: Timestamp,
    /// The length of a bucket in nanoseconds.
    pub step_ns: i64,
    pub services: Vec<ServiceSummary>,
    /// More services sent telemetry than the limit let through.
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ServiceSummary {
    pub service: String,
    /// The attributes of the newest resource of the service that has any,
    /// such as `telemetry.sdk.language`.
    pub resource: Attributes,
    pub stats: ServiceStats,
    /// Every step of the range, oldest first.
    pub buckets: Vec<ServiceBucket>,
}

/// One service in a range: its stats over time and by operation.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Service {
    pub service: String,
    /// The attributes of the newest resource of the service. Empty when the
    /// range has none of its telemetry.
    pub resource: Attributes,
    /// The range, after the retention capped it.
    #[schema(value_type = String, format = DateTime)]
    pub since: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub until: Timestamp,
    /// The length of a bucket in nanoseconds.
    pub step_ns: i64,
    pub stats: ServiceStats,
    /// Every step of the range, oldest first.
    pub buckets: Vec<ServiceBucket>,
    /// The requests by span name, the most first.
    pub operations: Vec<Operation>,
    /// The service has more operations than the limit let through.
    pub truncated: bool,
}

/// The requests and the logs of a service in a range.
#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema)]
pub struct ServiceStats {
    pub requests: Requests,
    pub logs: u64,
    /// The logs of the error level and above.
    pub error_logs: u64,
}

/// The requests and the logs of a service in one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ServiceBucket {
    /// The start of the step.
    #[schema(value_type = String, format = DateTime)]
    pub time: Timestamp,
    pub requests: Requests,
    pub logs: u64,
    pub error_logs: u64,
}

/// The requests of a service by the name of their span.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Operation {
    /// The span name, such as `GET /users/{id}`.
    pub name: String,
    /// The OpenTelemetry span kind.
    pub kind: i32,
    /// The attributes of its newest request, which tell what the operation
    /// is, such as `http.request.method` and `http.route`.
    pub attributes: Attributes,
    pub requests: Requests,
}

/// One operation of a service in a range: its requests over the range and
/// in buckets of one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct OperationDetail {
    pub service: String,
    /// The span name.
    pub name: String,
    /// The OpenTelemetry span kind.
    pub kind: i32,
    /// The attributes of its newest request. Empty when the range has none.
    pub attributes: Attributes,
    /// The range, after the retention capped it.
    #[schema(value_type = String, format = DateTime)]
    pub since: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub until: Timestamp,
    /// The length of a bucket in nanoseconds.
    pub step_ns: i64,
    pub requests: Requests,
    /// Every step of the range, oldest first.
    pub buckets: Vec<RequestBucket>,
}

/// The requests of one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct RequestBucket {
    /// The start of the step.
    #[schema(value_type = String, format = DateTime)]
    pub time: Timestamp,
    pub requests: Requests,
}

/// Spans counted together: the requests that enter a service, which are
/// roots and spans of the server or the consumer kind, or the calls it makes
/// to a database.
#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema)]
pub struct Requests {
    pub count: u64,
    /// The failed ones.
    pub errors: u64,
    /// Their durations added up.
    pub total_ns: i64,
    /// Estimates of the percentiles of their durations, off by 1% at most.
    /// `None` without requests.
    #[schema(required = true)]
    pub latency: Option<Latency>,
}

/// Percentiles of durations, in nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Latency {
    pub p50: i64,
    pub p95: i64,
    pub p99: i64,
}

/// The growth of the buckets of a [`Sketch`]. A value is taken for the middle
/// of its bucket, `2γ^i/(γ+1)`, which is off by `(γ-1)/(γ+1)`, about 1%.
const GAMMA: f64 = 1.02;

/// Counts of durations in buckets that each grow by [`GAMMA`], so a percentile
/// estimate keeps its relative error at any size, in memory that grows with
/// the logarithm of the range of the durations, not with their count.
#[derive(Default)]
struct Sketch {
    zeros: u64,
    counts: BTreeMap<i32, u64>,
}

impl Sketch {
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "the index of a bucket is below 2000 for any i64"
    )]
    fn add(&mut self, nanos: i64) {
        if nanos <= 0 {
            self.zeros += 1;
        } else {
            let index = (nanos as f64).log(GAMMA).ceil() as i32;
            *self.counts.entry(index).or_default() += 1;
        }
    }

    /// The smallest value that `q` of the `count` values are not above: the
    /// nearest rank.
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "an estimate"
    )]
    fn quantile(&self, q: f64, count: u64) -> i64 {
        let rank = ((q * count as f64).ceil() as u64).max(1) - 1;
        let mut seen = self.zeros;
        if rank < seen {
            return 0;
        }
        for (&index, &n) in &self.counts {
            seen += n;
            if rank < seen {
                return (2.0 * GAMMA.powi(index) / (GAMMA + 1.0)).round() as i64;
            }
        }
        0
    }
}

/// Adds up requests.
#[derive(Default)]
pub(super) struct Tally {
    count: u64,
    errors: u64,
    pub(super) total_ns: i64,
    sketch: Sketch,
}

impl Tally {
    pub(super) fn add(&mut self, duration_ns: i64, error: bool) {
        self.count += 1;
        self.errors += u64::from(error);
        self.total_ns = self.total_ns.saturating_add(duration_ns);
        self.sketch.add(duration_ns);
    }

    pub(super) fn finish(&self) -> Requests {
        Requests {
            count: self.count,
            errors: self.errors,
            total_ns: self.total_ns,
            latency: (self.count > 0).then(|| Latency {
                p50: self.sketch.quantile(0.5, self.count),
                p95: self.sketch.quantile(0.95, self.count),
                p99: self.sketch.quantile(0.99, self.count),
            }),
        }
    }
}

/// The requests and logs of one service, over the range and by step.
#[derive(Default)]
struct ServiceTally {
    requests: Tally,
    logs: u64,
    error_logs: u64,
    steps: HashMap<i64, (Tally, u64, u64)>,
}

impl ServiceTally {
    fn stats(&self) -> ServiceStats {
        ServiceStats {
            requests: self.requests.finish(),
            logs: self.logs,
            error_logs: self.error_logs,
        }
    }

    /// A bucket for every step from `first` up to `until`.
    fn buckets(&self, first: i64, until: i64, step: i64) -> Vec<ServiceBucket> {
        let empty = (Tally::default(), 0, 0);
        (first..until)
            .step_by(usize::try_from(step).unwrap_or(usize::MAX))
            .map(|start| {
                let (requests, logs, error_logs) = self.steps.get(&start).unwrap_or(&empty);
                ServiceBucket {
                    time: time(start),
                    requests: requests.finish(),
                    logs: *logs,
                    error_logs: *error_logs,
                }
            })
            .collect()
    }
}

/// Whose requests and logs a tally counts.
#[derive(Clone, Copy)]
enum Scope<'a> {
    Every,
    Service(&'a str),
    /// The requests of one operation of a service, without logs, which no
    /// operation has.
    Operation {
        service: &'a str,
        name: &'a str,
        kind: i32,
    },
}

impl<'a> Scope<'a> {
    const fn service(self) -> Option<&'a str> {
        match self {
            Self::Every => None,
            Self::Service(service) | Self::Operation { service, .. } => Some(service),
        }
    }
}

/// A span that entered a service.
struct Entry {
    service: String,
    name: String,
    kind: i32,
    start: i64,
    duration_ns: i64,
    error: bool,
    /// Where the span is: its day file and its row there.
    row: SpanRow,
}

/// A span by its day file and its row there.
pub(super) type SpanRow = (String, i64);

/// The requests of an operation, and the newest of them by its start.
#[derive(Default)]
pub(super) struct OperationTally {
    pub(super) requests: Tally,
    pub(super) newest: Option<(i64, SpanRow)>,
}

impl OperationTally {
    pub(super) fn add(&mut self, start: i64, duration_ns: i64, error: bool, row: &SpanRow) {
        self.requests.add(duration_ns, error);
        if self
            .newest
            .as_ref()
            .is_none_or(|(newest, _)| start >= *newest)
        {
            self.newest = Some((start, row.clone()));
        }
    }
}

impl Reader {
    /// The services that sent spans or logs in the range, `limit` at most,
    /// the most requests first, with their requests and logs in buckets of
    /// `step_ns`.
    ///
    /// # Errors
    ///
    /// When the step makes more than [`MAX_BUCKETS`] buckets, or when the
    /// query fails, such as when it runs past the time limit.
    pub fn services(&self, step_ns: i64, limit: usize) -> anyhow::Result<Services> {
        self.check_step(step_ns)?;
        let tallies = self.tally(Scope::Every, step_ns, |_| {})?;
        let mut resources = self.resources(None)?;
        let first = self.since().div_euclid(step_ns) * step_ns;
        let mut services: Vec<_> = tallies
            .into_iter()
            .map(|(service, tally)| ServiceSummary {
                resource: resources.remove(&service).unwrap_or_default(),
                stats: tally.stats(),
                buckets: tally.buckets(first, self.until(), step_ns),
                service,
            })
            .collect();
        services.sort_by(|a, b| {
            (b.stats.requests.count, b.stats.logs)
                .cmp(&(a.stats.requests.count, a.stats.logs))
                .then_with(|| a.service.cmp(&b.service))
        });
        let truncated = cut(&mut services, limit);
        Ok(Services {
            since: time(self.since()),
            until: time(self.until()),
            step_ns,
            services,
            truncated,
        })
    }

    /// The requests and the logs of `service` in the range, in buckets of
    /// `step_ns`, and its requests by span name, `limit` names at most.
    ///
    /// # Errors
    ///
    /// When the step makes more than [`MAX_BUCKETS`] buckets, or when the
    /// query fails, such as when it runs past the time limit.
    pub fn service(&self, service: &str, step_ns: i64, limit: usize) -> anyhow::Result<Service> {
        self.check_step(step_ns)?;
        let mut operations: HashMap<(String, i32), OperationTally> = HashMap::new();
        let mut tallies = self.tally(Scope::Service(service), step_ns, |entry| {
            operations
                .entry((entry.name.clone(), entry.kind))
                .or_default()
                .add(entry.start, entry.duration_ns, entry.error, &entry.row);
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
        let truncated = cut(&mut operations, limit);
        let mut attributes = self.span_attributes(
            operations
                .iter()
                .filter_map(|(_, o)| Some(&o.newest.as_ref()?.1)),
        )?;
        let operations = operations
            .into_iter()
            .map(|((name, kind), operation)| Operation {
                name,
                kind,
                attributes: operation
                    .newest
                    .and_then(|(_, row)| attributes.remove(&row))
                    .unwrap_or_default(),
                requests: operation.requests.finish(),
            })
            .collect();
        let first = self.since().div_euclid(step_ns) * step_ns;
        Ok(Service {
            service: service.to_owned(),
            resource: self
                .resources(Some(service))?
                .remove(service)
                .unwrap_or_default(),
            since: time(self.since()),
            until: time(self.until()),
            step_ns,
            stats: tally.stats(),
            buckets: tally.buckets(first, self.until(), step_ns),
            operations,
            truncated,
        })
    }

    /// The attributes of the spans at `rows`, read with one statement per
    /// day file.
    pub(super) fn span_attributes<'a>(
        &self,
        rows: impl Iterator<Item = &'a SpanRow>,
    ) -> anyhow::Result<HashMap<SpanRow, Attributes>> {
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
            let span = statement_span(&sql);
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

    /// The requests of the operation `name` of the kind `kind` of `service`
    /// in the range, over it and in buckets of `step_ns`.
    ///
    /// # Errors
    ///
    /// When the step makes more than [`MAX_BUCKETS`] buckets, or when the
    /// query fails, such as when it runs past the time limit.
    pub fn operation(
        &self,
        service: &str,
        name: &str,
        kind: i32,
        step_ns: i64,
    ) -> anyhow::Result<OperationDetail> {
        self.check_step(step_ns)?;
        let mut newest: Option<(i64, SpanRow)> = None;
        let scope = Scope::Operation {
            service,
            name,
            kind,
        };
        let mut tallies = self.tally(scope, step_ns, |entry| {
            if newest
                .as_ref()
                .is_none_or(|(start, _)| entry.start >= *start)
            {
                newest = Some((entry.start, entry.row.clone()));
            }
        })?;
        let tally = tallies.remove(service).unwrap_or_default();
        let attributes = match newest {
            Some((_, row)) => self.span_attributes(std::iter::once(&row))?.remove(&row),
            None => None,
        };
        let first = self.since().div_euclid(step_ns) * step_ns;
        Ok(OperationDetail {
            service: service.to_owned(),
            name: name.to_owned(),
            kind,
            attributes: attributes.unwrap_or_default(),
            since: time(self.since()),
            until: time(self.until()),
            step_ns,
            requests: tally.requests.finish(),
            buckets: tally
                .buckets(first, self.until(), step_ns)
                .into_iter()
                .map(|bucket| RequestBucket {
                    time: bucket.time,
                    requests: bucket.requests,
                })
                .collect(),
        })
    }

    pub(super) fn check_step(&self, step_ns: i64) -> anyhow::Result<()> {
        ensure!(step_ns > 0, "the step has to be longer than zero");
        ensure!(
            (self.until() - self.since()) / step_ns <= MAX_BUCKETS,
            "the step makes more than {MAX_BUCKETS} buckets in the range; raise the step"
        );
        Ok(())
    }

    /// The requests and the logs of the services in `scope`, with each
    /// request also handed to `each`.
    fn tally(
        &self,
        scope: Scope,
        step: i64,
        mut each: impl FnMut(&Entry),
    ) -> anyhow::Result<HashMap<String, ServiceTally>> {
        let mut tallies: HashMap<String, ServiceTally> = HashMap::new();

        let mut where_ = Filter::range(self, "s.start_ts");
        where_.push_clause(ENTRY.into());
        if let Some(service) = scope.service() {
            where_.push("r.service = :service", ":service", service.to_owned());
        }
        if let Scope::Operation { name, kind, .. } = scope {
            where_.push("s.name = :name", ":name", name.to_owned());
            where_.push("s.kind = :kind", ":kind", kind);
        }
        self.scan(
            ["", ""],
            |day| {
                format!(
                    "SELECT r.service, s.name, s.kind, s.start_ts, s.duration_ns, s.status,
                            '{}', s.rowid
                     FROM {day}.spans s JOIN {day}.resources r ON r.id = s.resource_id
                     WHERE {}",
                    day.trim_matches('"'),
                    where_.sql(day)
                )
            },
            &where_,
            |row| {
                let entry = Entry {
                    service: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    start: row.get(3)?,
                    duration_ns: row.get(4)?,
                    error: row.get::<_, i32>(5)? == STATUS_ERROR,
                    row: (row.get(6)?, row.get(7)?),
                };
                let tally = tallies.entry(entry.service.clone()).or_default();
                tally.requests.add(entry.duration_ns, entry.error);
                let start = entry.start.div_euclid(step) * step;
                let (requests, _, _) = tally.steps.entry(start).or_default();
                requests.add(entry.duration_ns, entry.error);
                each(&entry);
                Ok(true)
            },
        )?;

        if matches!(scope, Scope::Operation { .. }) {
            return Ok(tallies);
        }
        let mut where_ = Filter::range(self, "l.ts");
        where_.param(":step", step);
        if let Some(service) = scope.service() {
            where_.push("r.service = :service", ":service", service.to_owned());
        }
        self.scan(
            [
                "SELECT service, start, sum(n), sum(errors) FROM (",
                ") GROUP BY service, start",
            ],
            |day| {
                format!(
                    "SELECT r.service, l.ts / :step * :step AS start, count(*) AS n,
                            sum(l.severity >= {ERROR_SEVERITY}) AS errors
                     FROM {day}.logs l JOIN {day}.resources r ON r.id = l.resource_id
                     WHERE {}
                     GROUP BY 1, 2",
                    where_.sql(day)
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

    /// The attributes of the newest resource of each service, or of `only`
    /// that one, in the day files of the range. A resource with attributes
    /// wins over one without, such as the one of the writer's own counters.
    fn resources(&self, only: Option<&str>) -> anyhow::Result<HashMap<String, Attributes>> {
        let mut where_ = Filter::new();
        if let Some(service) = only {
            where_.push("service = :service", ":service", service.to_owned());
        }
        let mut resources = HashMap::new();
        self.scan(
            [
                "SELECT service, attributes FROM (",
                ") ORDER BY attributes != '{}', day, id",
            ],
            |day| {
                format!(
                    "SELECT '{}' AS day, id, service, attributes FROM {day}.resources WHERE {}",
                    day.trim_matches('"'),
                    where_.sql(day)
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
