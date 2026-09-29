//! `siner services`, `siner service`, and `siner calls`.

use std::io;

use jiff::Timestamp;
use serde::Serialize;
use siner_telemetry::Attributes;
use siner_telemetry::query::{
    CallOperation, Calls, Operation, RequestBucket, Requests, Service, ServiceBucket, ServiceStats,
    Services, Target, TargetKey, TargetType,
};

use super::Range;
use super::client::{Client, note_cut, path_segment, print_json};
use super::table::{self, Table};

#[derive(clap::Args)]
pub struct ServicesArgs {
    /// Print the requests and the logs of each step too
    #[arg(long)]
    buckets: bool,

    /// The length of a step, such as 1m [default: one that makes 60 steps at
    /// most]
    #[arg(long)]
    step: Option<String>,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

/// Runs `siner services`.
///
/// # Errors
///
/// When the daemon cannot be reached, or answers with an error.
pub fn services(args: &ServicesArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.push(("step", args.step.clone()));
    let list: Services = args.client.get("/api/services", &params)?;
    if args.client.wants_table() {
        let mut table = Table::new(&[
            "SERVICE",
            "REQUESTS",
            "ERRORS",
            "P50",
            "P95",
            "P99",
            "LOGS",
            "ERROR LOGS",
        ]);
        for service in &list.services {
            let mut cells = vec![service.service.clone()];
            cells.extend(request_cells(&service.stats.requests));
            cells.extend([
                service.stats.logs.to_string(),
                service.stats.error_logs.to_string(),
            ]);
            table.row(cells);
        }
        table.print()?;
        if args.buckets {
            for service in &list.services {
                println!();
                println!(
                    "{} every {}",
                    service.service,
                    table::duration(list.step_ns)
                );
                print_steps(&service.buckets)?;
            }
        }
    } else if args.buckets {
        print_json(&list)?;
    } else {
        print_json(&ServicesTotals::of(&list))?;
    }
    note_cut(
        list.truncated,
        "More services sent telemetry; raise --limit.",
    );
    Ok(())
}

#[derive(clap::Args)]
pub struct ServiceArgs {
    /// The name of the service
    name: String,

    /// Print the requests and the logs of each step too
    #[arg(long)]
    buckets: bool,

    /// The length of a step, such as 1m [default: one that makes 120 steps at
    /// most]
    #[arg(long)]
    step: Option<String>,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

/// Runs `siner service`.
///
/// # Errors
///
/// When the daemon cannot be reached, or answers with an error.
pub fn service(args: &ServiceArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.push(("step", args.step.clone()));
    let service: Service = args.client.get(
        &format!("/api/services/{}", path_segment(&args.name)),
        &params,
    )?;
    if args.client.wants_table() {
        let requests = &service.stats.requests;
        println!(
            "{}: {} requests, {} errors, {} logs, {} error logs",
            service.service,
            requests.count,
            requests.errors,
            service.stats.logs,
            service.stats.error_logs,
        );
        if !service.operations.is_empty() {
            println!();
            let mut table = Table::new(&[
                "OPERATION",
                "REQUESTS",
                "ERRORS",
                "P50",
                "P95",
                "P99",
                "TOTAL",
            ]);
            for operation in &service.operations {
                let mut cells = vec![operation.name.clone()];
                cells.extend(request_cells(&operation.requests));
                cells.push(table::duration(operation.requests.total_ns));
                table.row(cells);
            }
            table.print()?;
        }
        if args.buckets {
            println!();
            println!("every {}", table::duration(service.step_ns));
            print_steps(&service.buckets)?;
        }
    } else if args.buckets {
        print_json(&service)?;
    } else {
        print_json(&ServiceTotals::of(&service))?;
    }
    note_cut(
        service.truncated,
        "The service has more operations; raise --limit.",
    );
    Ok(())
}

#[derive(clap::Args)]
pub struct CallsArgs {
    /// The name of the service
    name: String,

    /// Print the calls of each step too
    #[arg(long)]
    buckets: bool,

    /// The length of a step, such as 1m [default: one that makes 120 steps at
    /// most]
    #[arg(long)]
    step: Option<String>,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

/// Runs `siner calls`.
///
/// # Errors
///
/// When the daemon cannot be reached, or answers with an error.
pub fn calls(args: &CallsArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.push(("step", args.step.clone()));
    let answer: Calls = args.client.get(
        &format!("/api/services/{}/calls", path_segment(&args.name)),
        &params,
    )?;
    if args.client.wants_table() {
        println!(
            "{}: {} calls, {} errors, {} in all",
            answer.service,
            answer.calls.count,
            answer.calls.errors,
            table::duration(answer.calls.total_ns),
        );
        if !answer.targets.is_empty() {
            println!();
            let mut table =
                Table::new(&["TARGET", "CALLS", "ERRORS", "P50", "P95", "P99", "TOTAL"]);
            for target in &answer.targets {
                let mut cells = vec![target_label(&target.key)];
                cells.extend(request_cells(&target.calls));
                cells.push(table::duration(target.calls.total_ns));
                table.row(cells);
            }
            table.print()?;

            println!();
            let mut table = Table::new(&[
                "TARGET", "CALL", "CALLS", "ERRORS", "P50", "P95", "P99", "TOTAL",
            ]);
            let mut operations: Vec<_> = answer
                .targets
                .iter()
                .flat_map(|t| t.operations.iter().map(move |o| (&t.key, o)))
                .collect();
            operations.sort_by_key(|(_, o)| std::cmp::Reverse(o.calls.total_ns));
            for (key, operation) in operations {
                let mut cells = vec![target_label(key), operation.summary.clone()];
                cells.extend(request_cells(&operation.calls));
                cells.push(table::duration(operation.calls.total_ns));
                table.row(cells);
            }
            table.print()?;
        }
        if args.buckets {
            println!();
            println!("every {}", table::duration(answer.step_ns));
            print_call_steps(&answer.buckets)?;
        }
    } else if args.buckets {
        print_json(&answer)?;
    } else {
        print_json(&CallsTotals::of(&answer))?;
    }
    note_cut(
        answer.truncated,
        "The service makes more kinds of calls; raise --limit.",
    );
    Ok(())
}

/// A target as the system or the type, and the name, such as
/// `postgresql app` or `http api.stripe.com`.
fn target_label(key: &TargetKey) -> String {
    let kind = key.system.clone().unwrap_or_else(|| {
        match key.target_type {
            TargetType::Database => "database",
            TargetType::Http => "http",
            TargetType::Rpc => "rpc",
            TargetType::Messaging => "messaging",
            TargetType::Other => "other",
        }
        .to_owned()
    });
    match &key.name {
        Some(name) => format!("{kind} {name}"),
        None => kind,
    }
}

/// The calls of each step.
fn print_call_steps(buckets: &[RequestBucket]) -> io::Result<()> {
    let mut table = Table::new(&["TIME (UTC)", "CALLS", "ERRORS", "P50", "P95", "P99"]);
    for bucket in buckets {
        let mut cells = vec![table::time(bucket.time)];
        cells.extend(request_cells(&bucket.requests));
        table.row(cells);
    }
    table.print()
}

/// The count, the errors, and the percentiles of requests, with dashes for
/// the percentiles when there are none.
fn request_cells(requests: &Requests) -> [String; 5] {
    let percentile = |pick: fn(&siner_telemetry::query::Latency) -> i64| {
        requests
            .latency
            .as_ref()
            .map_or_else(|| "-".into(), |latency| table::duration(pick(latency)))
    };
    [
        requests.count.to_string(),
        requests.errors.to_string(),
        percentile(|l| l.p50),
        percentile(|l| l.p95),
        percentile(|l| l.p99),
    ]
}

/// The requests and the logs of each step.
fn print_steps(buckets: &[ServiceBucket]) -> io::Result<()> {
    let mut table = Table::new(&[
        "TIME (UTC)",
        "REQUESTS",
        "ERRORS",
        "P50",
        "P95",
        "P99",
        "LOGS",
        "ERROR LOGS",
    ]);
    for bucket in buckets {
        let mut cells = vec![table::time(bucket.time)];
        cells.extend(request_cells(&bucket.requests));
        cells.extend([bucket.logs.to_string(), bucket.error_logs.to_string()]);
        table.row(cells);
    }
    table.print()
}

/// The services over the range without their steps, which `--buckets` adds,
/// so the JSON stays short.
#[derive(Serialize)]
struct ServicesTotals<'a> {
    since: Timestamp,
    until: Timestamp,
    services: Vec<ServiceSummaryTotals<'a>>,
    truncated: bool,
}

#[derive(Serialize)]
struct ServiceSummaryTotals<'a> {
    service: &'a str,
    resource: &'a Attributes,
    stats: &'a ServiceStats,
}

impl<'a> ServicesTotals<'a> {
    fn of(list: &'a Services) -> Self {
        Self {
            since: list.since,
            until: list.until,
            services: list
                .services
                .iter()
                .map(|service| ServiceSummaryTotals {
                    service: &service.service,
                    resource: &service.resource,
                    stats: &service.stats,
                })
                .collect(),
            truncated: list.truncated,
        }
    }
}

/// One service over the range without its steps, which `--buckets` adds.
#[derive(Serialize)]
struct ServiceTotals<'a> {
    service: &'a str,
    resource: &'a Attributes,
    since: Timestamp,
    until: Timestamp,
    stats: &'a ServiceStats,
    operations: &'a [Operation],
    truncated: bool,
}

impl<'a> ServiceTotals<'a> {
    fn of(service: &'a Service) -> Self {
        Self {
            service: &service.service,
            resource: &service.resource,
            since: service.since,
            until: service.until,
            stats: &service.stats,
            operations: &service.operations,
            truncated: service.truncated,
        }
    }
}

/// The calls over the range without their steps, which `--buckets` adds.
#[derive(Serialize)]
struct CallsTotals<'a> {
    service: &'a str,
    since: Timestamp,
    until: Timestamp,
    calls: &'a Requests,
    targets: Vec<TargetTotals<'a>>,
    truncated: bool,
}

#[derive(Serialize)]
struct TargetTotals<'a> {
    #[serde(flatten)]
    key: &'a TargetKey,
    query: &'a str,
    calls: &'a Requests,
    operations: &'a [CallOperation],
}

impl<'a> CallsTotals<'a> {
    fn of(calls: &'a Calls) -> Self {
        Self {
            service: &calls.service,
            since: calls.since,
            until: calls.until,
            calls: &calls.calls,
            targets: calls
                .targets
                .iter()
                .map(|target: &'a Target| TargetTotals {
                    key: &target.key,
                    query: &target.query,
                    calls: &target.calls,
                    operations: &target.operations,
                })
                .collect(),
            truncated: calls.truncated,
        }
    }
}
