use std::io;

use jiff::Timestamp;
use otelo_storage::Attributes;
use otelo_storage::query::{
    CallOperation, Calls, Operation, RequestBucket, Requests, Service, ServiceBucket, ServiceStats,
    Services, Target, TargetKey, TargetType,
};
use serde::Serialize;

use super::Range;
use super::client::{Client, escape_path_segment, note_cut, print_json};
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
                    table::format_duration(list.step_ns)
                );
                print_steps(&service.buckets)?;
            }
        }
    } else if args.buckets {
        print_json(&list)?;
    } else {
        print_json(&ServicesWithoutBuckets::of(&list))?;
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

pub fn service(args: &ServiceArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.push(("step", args.step.clone()));
    let service: Service = args.client.get(
        &format!("/api/services/{}", escape_path_segment(&args.name)),
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
                cells.push(table::format_duration(operation.requests.total_ns));
                table.row(cells);
            }
            table.print()?;
        }
        if args.buckets {
            println!();
            println!("every {}", table::format_duration(service.step_ns));
            print_steps(&service.buckets)?;
        }
    } else if args.buckets {
        print_json(&service)?;
    } else {
        print_json(&ServiceWithoutBuckets::of(&service))?;
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

pub fn calls(args: &CallsArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.push(("step", args.step.clone()));
    let answer: Calls = args.client.get(
        &format!("/api/services/{}/calls", escape_path_segment(&args.name)),
        &params,
    )?;
    if args.client.wants_table() {
        println!(
            "{}: {} calls, {} errors, {} in all",
            answer.service,
            answer.calls.count,
            answer.calls.errors,
            table::format_duration(answer.calls.total_ns),
        );
        if !answer.targets.is_empty() {
            println!();
            let mut table =
                Table::new(&["TARGET", "CALLS", "ERRORS", "P50", "P95", "P99", "TOTAL"]);
            for target in &answer.targets {
                let mut cells = vec![target_label(&target.key)];
                cells.extend(request_cells(&target.calls));
                cells.push(table::format_duration(target.calls.total_ns));
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
                cells.push(table::format_duration(operation.calls.total_ns));
                table.row(cells);
            }
            table.print()?;
        }
        if args.buckets {
            println!();
            println!("every {}", table::format_duration(answer.step_ns));
            print_call_steps(&answer.buckets)?;
        }
    } else if args.buckets {
        print_json(&answer)?;
    } else {
        print_json(&CallsWithoutBuckets::of(&answer))?;
    }
    note_cut(
        answer.truncated,
        "The service makes more kinds of calls; raise --limit.",
    );
    Ok(())
}

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

fn print_call_steps(buckets: &[RequestBucket]) -> io::Result<()> {
    let mut table = Table::new(&["TIME (UTC)", "CALLS", "ERRORS", "P50", "P95", "P99"]);
    for bucket in buckets {
        let mut cells = vec![table::format_utc_time(bucket.start_at)];
        cells.extend(request_cells(&bucket.requests));
        table.row(cells);
    }
    table.print()
}

fn request_cells(requests: &Requests) -> [String; 5] {
    let percentile = |pick: fn(&otelo_storage::query::Latency) -> i64| {
        requests.latency.as_ref().map_or_else(
            || "-".into(),
            |latency| table::format_duration(pick(latency)),
        )
    };
    [
        requests.count.to_string(),
        requests.errors.to_string(),
        percentile(|l| l.p50),
        percentile(|l| l.p95),
        percentile(|l| l.p99),
    ]
}

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
        let mut cells = vec![table::format_utc_time(bucket.start_at)];
        cells.extend(request_cells(&bucket.requests));
        cells.extend([bucket.logs.to_string(), bucket.error_logs.to_string()]);
        table.row(cells);
    }
    table.print()
}

#[derive(Serialize)]
struct ServicesWithoutBuckets<'a> {
    start_at: Timestamp,
    end_at: Timestamp,
    services: Vec<ServiceSummaryWithoutBuckets<'a>>,
    truncated: bool,
}

#[derive(Serialize)]
struct ServiceSummaryWithoutBuckets<'a> {
    service: &'a str,
    resource: &'a Attributes,
    stats: &'a ServiceStats,
}

impl<'a> ServicesWithoutBuckets<'a> {
    fn of(list: &'a Services) -> Self {
        Self {
            start_at: list.start_at,
            end_at: list.end_at,
            services: list
                .services
                .iter()
                .map(|service| ServiceSummaryWithoutBuckets {
                    service: &service.service,
                    resource: &service.resource,
                    stats: &service.stats,
                })
                .collect(),
            truncated: list.truncated,
        }
    }
}

#[derive(Serialize)]
struct ServiceWithoutBuckets<'a> {
    service: &'a str,
    resource: &'a Attributes,
    start_at: Timestamp,
    end_at: Timestamp,
    stats: &'a ServiceStats,
    operations: &'a [Operation],
    truncated: bool,
}

impl<'a> ServiceWithoutBuckets<'a> {
    fn of(service: &'a Service) -> Self {
        Self {
            service: &service.service,
            resource: &service.resource,
            start_at: service.start_at,
            end_at: service.end_at,
            stats: &service.stats,
            operations: &service.operations,
            truncated: service.truncated,
        }
    }
}

#[derive(Serialize)]
struct CallsWithoutBuckets<'a> {
    service: &'a str,
    start_at: Timestamp,
    end_at: Timestamp,
    calls: &'a Requests,
    targets: Vec<TargetWithoutBuckets<'a>>,
    truncated: bool,
}

#[derive(Serialize)]
struct TargetWithoutBuckets<'a> {
    #[serde(flatten)]
    key: &'a TargetKey,
    query: &'a str,
    calls: &'a Requests,
    operations: &'a [CallOperation],
}

impl<'a> CallsWithoutBuckets<'a> {
    fn of(calls: &'a Calls) -> Self {
        Self {
            service: &calls.service,
            start_at: calls.start_at,
            end_at: calls.end_at,
            calls: &calls.calls,
            targets: calls
                .targets
                .iter()
                .map(|target: &'a Target| TargetWithoutBuckets {
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
