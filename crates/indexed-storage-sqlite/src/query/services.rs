use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::iter;
use std::ops::ControlFlow;

use anyhow::ensure;
use otelo_indexed_storage::query::{
    MAX_BUCKETS_IN_RANGE, Service, ServiceBucket, ServiceStats, ServiceSummary, Services,
    SpanGroupingField,
};
use otelo_indexed_storage::{Attributes, Severity, SpanKind, SpanStatus};

use otelo_query::{BuiltinField, Expression, Field, Operator, Query, Signal, Value};

use super::span_stats::SpanTally;
use super::traces::compile_span_query;
use super::{LOG_ALIASES, SPAN_ALIASES, WhereClause, timestamp_from_nanos, truncate_to_limit};
use crate::Reader;
use crate::series::ResourceId;

fn http_request_query() -> Query {
    Query {
        signal: Signal::Spans,
        expression: Some(Expression::And(vec![
            Expression::Compare {
                field: Field::Builtin(BuiltinField::Kind),
                operator: Operator::Eq,
                value: Value::String(SpanKind::Server.name().to_owned()),
            },
            Expression::Has(Field::Attribute("http.request.method".to_owned())),
        ])),
    }
}

#[derive(Default)]
struct StepTally {
    requests: SpanTally,
    spans: u64,
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
            requests: self.total.requests.to_span_stats(),
            spans: self.total.spans,
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
                    requests: step.requests.to_span_stats(),
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
}

impl<'a> TallyScope<'a> {
    const fn service(self) -> Option<&'a str> {
        match self {
            Self::EveryService => None,
            Self::Service(service) => Some(service),
        }
    }
}

impl Reader {
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
    ) -> anyhow::Result<HashMap<String, ServiceTally>> {
        let mut tallies: HashMap<String, ServiceTally> = HashMap::new();
        if self.tally_services_from_summaries(scope, step_ns, &mut tallies)? {
            self.tally_logs(scope.service(), step_ns, &mut tallies)?;
            return Ok(tallies);
        }

        let (mut where_clause, _) = compile_span_query(self, &http_request_query())?;
        if let Some(service) = scope.service() {
            where_clause.push_condition_with_param(
                "resource.service = :service",
                ":service",
                service.to_owned(),
            );
        }
        let sql = format!(
            "SELECT resource.service, span.started_at, span.duration_ns, span.status_code
             FROM {}
             WHERE {}",
            SPAN_ALIASES.encoded_records_sql("spans"),
            where_clause.sql()
        );
        self.scan_rows(&sql, &where_clause, |row| {
            let started_at: i64 = row.get(1)?;
            let duration_ns: i64 = row.get(2)?;
            let failed = SpanStatus::from_number(row.get(3)?).is_error();
            let tally = tallies.entry(row.get(0)?).or_default();
            tally.total.requests.add_span(duration_ns, failed);
            let step_at = started_at.div_euclid(step_ns) * step_ns;
            tally
                .steps
                .entry(step_at)
                .or_default()
                .requests
                .add_span(duration_ns, failed);
            Ok(ControlFlow::Continue(()))
        })?;
        self.count_spans(scope.service(), &mut tallies)?;
        self.tally_logs(scope.service(), step_ns, &mut tallies)?;
        Ok(tallies)
    }

    fn tally_services_from_summaries(
        &self,
        scope: TallyScope,
        step_ns: i64,
        tallies: &mut HashMap<String, ServiceTally>,
    ) -> anyhow::Result<bool> {
        let scope_expression = scope.service().map(|service| Expression::Compare {
            field: Field::Builtin(BuiltinField::Service),
            operator: Operator::Eq,
            value: Value::String(service.to_owned()),
        });
        let Some(request_expression) = http_request_query().expression else {
            return Ok(false);
        };
        let request_expression = Expression::And(
            iter::once(request_expression)
                .chain(scope_expression.clone())
                .collect(),
        );
        let request_query = Query {
            signal: Signal::Spans,
            expression: Some(request_expression),
        };
        let Some(request_rows) =
            self.read_span_summary_rows(&request_query, &[SpanGroupingField::Service], step_ns)?
        else {
            return Ok(false);
        };
        let span_query = Query {
            signal: Signal::Spans,
            expression: scope_expression,
        };
        let Some(span_rows) =
            self.read_span_summary_rows(&span_query, &[SpanGroupingField::Service], step_ns)?
        else {
            return Ok(false);
        };
        for row in &request_rows {
            let tally = tallies.entry(row.key.service.to_string()).or_default();
            tally.total.requests.add_summary(&row.summary, row.failed());
            tally
                .steps
                .entry(row.start_at.div_euclid(step_ns) * step_ns)
                .or_default()
                .requests
                .add_summary(&row.summary, row.failed());
        }
        for row in &span_rows {
            tallies
                .entry(row.key.service.to_string())
                .or_default()
                .total
                .spans += row.summary.span_count;
        }
        Ok(true)
    }

    fn count_spans(
        &self,
        only_service: Option<&str>,
        tallies: &mut HashMap<String, ServiceTally>,
    ) -> anyhow::Result<()> {
        let mut where_clause = WhereClause::within_reader_range(self, "span.started_at");
        if let Some(service) = only_service {
            where_clause.push_condition_with_param(
                "resource.service = :service",
                ":service",
                service.to_owned(),
            );
        }
        let sql = format!(
            "SELECT resource.service, count(*)
             FROM {}
             WHERE {}
             GROUP BY resource.service",
            SPAN_ALIASES.encoded_records_sql("spans"),
            where_clause.sql()
        );
        self.scan_rows(&sql, &where_clause, |row| {
            let spans = u64::try_from(row.get::<_, i64>(1)?)?;
            tallies.entry(row.get(0)?).or_default().total.spans += spans;
            Ok(ControlFlow::Continue(()))
        })
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
             FROM {}
             WHERE {}
             GROUP BY resource.service, step_start_at",
            Severity::ERROR.number(),
            LOG_ALIASES.encoded_records_sql("logs"),
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
                "resource.service = :service",
                ":service",
                service.to_owned(),
            );
        }
        // Metrics don't pick the resource, because otelo's host collector sends those of a service
        // under a resource of its own, without the attributes of the service's SDK.
        let sql = format!(
            "SELECT resource.service,
                    resource.attributes,
                    resource.id,
                    max(coalesce((SELECT max(span.started_at)
                                  FROM spans span
                                  WHERE span.stable_attribute_set_id = stable_attribute_set.id),
                                 (SELECT max(log.logged_at)
                                  FROM logs log
                                  WHERE log.stable_attribute_set_id = stable_attribute_set.id)))
                        AS newest_log_or_span_at
             FROM resources resource
             JOIN record_groups record_group ON record_group.resource_id = resource.id
             JOIN stable_attribute_sets stable_attribute_set
               ON stable_attribute_set.record_group_id = record_group.id
             WHERE {}
             GROUP BY resource.id",
            where_clause.sql()
        );
        let mut picked_resources: HashMap<String, ResourceCandidate> = HashMap::new();
        self.scan_rows(&sql, &where_clause, |row| {
            let Some(newest_log_or_span_at) = row.get(3)? else {
                return Ok(ControlFlow::Continue(()));
            };
            let attributes: String = row.get(1)?;
            let candidate = ResourceCandidate {
                attributes: serde_json::from_str(&attributes)?,
                newest_log_or_span_at,
                resource_id: row.get(2)?,
            };
            match picked_resources.entry(row.get(0)?) {
                Entry::Occupied(mut entry) => {
                    if entry.get().rank() < candidate.rank() {
                        entry.insert(candidate);
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(candidate);
                }
            }
            Ok(ControlFlow::Continue(()))
        })?;
        Ok(picked_resources
            .into_iter()
            .map(|(service, candidate)| (service, candidate.attributes))
            .collect())
    }
}

struct ResourceCandidate {
    attributes: Attributes,
    newest_log_or_span_at: i64,
    resource_id: ResourceId,
}

impl ResourceCandidate {
    fn rank(&self) -> (bool, i64, ResourceId) {
        (
            !self.attributes.is_empty(),
            self.newest_log_or_span_at,
            self.resource_id,
        )
    }
}

pub(super) fn summarize_services(
    reader: &Reader,
    step_ns: i64,
    limit: usize,
) -> anyhow::Result<Services> {
    reader.check_step(step_ns)?;
    let tallies = reader.tally_services(TallyScope::EveryService, step_ns)?;
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
) -> anyhow::Result<Service> {
    reader.check_step(step_ns)?;
    let tally = reader
        .tally_services(TallyScope::Service(service), step_ns)?
        .remove(service)
        .unwrap_or_default();
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
    })
}
