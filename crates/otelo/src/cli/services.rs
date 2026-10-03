use std::io;

use jiff::Timestamp;
use otelo_indexed_storage::Attributes;
use otelo_indexed_storage::query::{
    CallOperation, Calls, Operation, RequestBucket, Requests, Service, ServiceBucket, ServiceStats,
    Services, Target, TargetKey, TargetType,
};
use serde::Serialize;

use super::RangeArgs;
use super::client::{Client, OutputFormat, escape_path_segment, note_truncation, print_json};
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
    range: RangeArgs,

    #[command(flatten)]
    client: Client,
}

pub fn print_services(args: &ServicesArgs) -> anyhow::Result<()> {
    let mut params = args.range.to_query_params();
    params.push(("step", args.step.clone()));
    let answer: Services = args.client.get("/api/services", &params)?;
    match args.client.choose_output_format() {
        OutputFormat::Table => {
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
            for service in &answer.services {
                let mut cells = vec![service.service.clone()];
                cells.extend(format_request_cells(&service.stats.requests));
                cells.extend([
                    service.stats.logs.to_string(),
                    service.stats.error_logs.to_string(),
                ]);
                table.add_row(cells);
            }
            table.print()?;
            if args.buckets {
                for service in &answer.services {
                    println!();
                    println!(
                        "{} every {}",
                        service.service,
                        table::format_duration(answer.step_ns)
                    );
                    print_service_steps(&service.buckets)?;
                }
            }
        }
        OutputFormat::Json if args.buckets => print_json(&answer)?,
        OutputFormat::Json => print_json(&ServicesWithoutBuckets::from(&answer))?,
    }
    note_truncation(
        answer.truncated,
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
    range: RangeArgs,

    #[command(flatten)]
    client: Client,
}

pub fn print_service(args: &ServiceArgs) -> anyhow::Result<()> {
    let mut params = args.range.to_query_params();
    params.push(("step", args.step.clone()));
    let service: Service = args.client.get(
        &format!("/api/services/{}", escape_path_segment(&args.name)),
        &params,
    )?;
    match args.client.choose_output_format() {
        OutputFormat::Table => {
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
                    cells.extend(format_request_cells(&operation.requests));
                    cells.push(table::format_duration(operation.requests.total_ns));
                    table.add_row(cells);
                }
                table.print()?;
            }
            if args.buckets {
                println!();
                println!("every {}", table::format_duration(service.step_ns));
                print_service_steps(&service.buckets)?;
            }
        }
        OutputFormat::Json if args.buckets => print_json(&service)?,
        OutputFormat::Json => print_json(&ServiceWithoutBuckets::from(&service))?,
    }
    note_truncation(
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
    range: RangeArgs,

    #[command(flatten)]
    client: Client,
}

pub fn print_calls(args: &CallsArgs) -> anyhow::Result<()> {
    let mut params = args.range.to_query_params();
    params.push(("step", args.step.clone()));
    let answer: Calls = args.client.get(
        &format!("/api/services/{}/calls", escape_path_segment(&args.name)),
        &params,
    )?;
    match args.client.choose_output_format() {
        OutputFormat::Table => {
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
                    let mut cells = vec![format_target_label(&target.key)];
                    cells.extend(format_request_cells(&target.calls));
                    cells.push(table::format_duration(target.calls.total_ns));
                    table.add_row(cells);
                }
                table.print()?;

                println!();
                let mut table = Table::new(&[
                    "TARGET", "CALL", "CALLS", "ERRORS", "P50", "P95", "P99", "TOTAL",
                ]);
                let mut operations: Vec<_> = answer
                    .targets
                    .iter()
                    .flat_map(|target| {
                        target
                            .operations
                            .iter()
                            .map(move |operation| (&target.key, operation))
                    })
                    .collect();
                operations
                    .sort_by_key(|(_, operation)| std::cmp::Reverse(operation.calls.total_ns));
                for (key, operation) in operations {
                    let mut cells = vec![format_target_label(key), operation.summary.clone()];
                    cells.extend(format_request_cells(&operation.calls));
                    cells.push(table::format_duration(operation.calls.total_ns));
                    table.add_row(cells);
                }
                table.print()?;
            }
            if args.buckets {
                println!();
                println!("every {}", table::format_duration(answer.step_ns));
                print_call_steps(&answer.buckets)?;
            }
        }
        OutputFormat::Json if args.buckets => print_json(&answer)?,
        OutputFormat::Json => print_json(&CallsWithoutBuckets::from(&answer))?,
    }
    note_truncation(
        answer.truncated,
        "The service makes more kinds of calls; raise --limit.",
    );
    Ok(())
}

fn format_target_label(key: &TargetKey) -> String {
    let system_or_type = key.system.clone().unwrap_or_else(|| {
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
        Some(name) => format!("{system_or_type} {name}"),
        None => system_or_type,
    }
}

fn print_call_steps(buckets: &[RequestBucket]) -> io::Result<()> {
    let mut table = Table::new(&["TIME (UTC)", "CALLS", "ERRORS", "P50", "P95", "P99"]);
    for bucket in buckets {
        let mut cells = vec![table::format_utc_time(bucket.start_at)];
        cells.extend(format_request_cells(&bucket.requests));
        table.add_row(cells);
    }
    table.print()
}

fn format_request_cells(requests: &Requests) -> [String; 5] {
    let format_percentile = |pick: fn(&otelo_indexed_storage::query::Latency) -> i64| {
        requests.latency.as_ref().map_or_else(
            || "-".into(),
            |latency| table::format_duration(pick(latency)),
        )
    };
    [
        requests.count.to_string(),
        requests.errors.to_string(),
        format_percentile(|latency| latency.p50),
        format_percentile(|latency| latency.p95),
        format_percentile(|latency| latency.p99),
    ]
}

fn print_service_steps(buckets: &[ServiceBucket]) -> io::Result<()> {
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
        cells.extend(format_request_cells(&bucket.requests));
        cells.extend([bucket.logs.to_string(), bucket.error_logs.to_string()]);
        table.add_row(cells);
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

impl<'a> From<&'a Services> for ServicesWithoutBuckets<'a> {
    fn from(answer: &'a Services) -> Self {
        Self {
            start_at: answer.start_at,
            end_at: answer.end_at,
            services: answer
                .services
                .iter()
                .map(|service| ServiceSummaryWithoutBuckets {
                    service: &service.service,
                    resource: &service.resource,
                    stats: &service.stats,
                })
                .collect(),
            truncated: answer.truncated,
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

impl<'a> From<&'a Service> for ServiceWithoutBuckets<'a> {
    fn from(service: &'a Service) -> Self {
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

impl<'a> From<&'a Calls> for CallsWithoutBuckets<'a> {
    fn from(answer: &'a Calls) -> Self {
        Self {
            service: &answer.service,
            start_at: answer.start_at,
            end_at: answer.end_at,
            calls: &answer.calls,
            targets: answer
                .targets
                .iter()
                .map(|target: &'a Target| TargetWithoutBuckets {
                    key: &target.key,
                    query: &target.query,
                    calls: &target.calls,
                    operations: &target.operations,
                })
                .collect(),
            truncated: answer.truncated,
        }
    }
}
