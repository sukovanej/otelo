use std::io;

use jiff::Timestamp;
use otelo_indexed_storage::Attributes;
use otelo_indexed_storage::query::{
    Service, ServiceBucket, ServiceStats, Services, SpanGroup, SpanGroups,
};
use otelo_query::quote_string;
use serde::Serialize;

use super::RangeArgs;
use super::client::{Client, OutputFormat, escape_path_segment, note_truncation, print_json};
use super::span_groups::{format_group_value, format_span_stats_cells, print_span_group_table};
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
                cells.extend(format_span_stats_cells(&service.stats.requests));
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
    let mut params = args.range.to_range_params();
    params.push(("step", args.step.clone()));
    let service: Service = args.client.get(
        &format!("/api/services/{}", escape_path_segment(&args.name)),
        &params,
    )?;
    let service_term = format!("service = {}", quote_string(&args.name));
    let routes = fetch_span_groups(
        args,
        &format!("{service_term} kind = server has(http.request.method)"),
        "http.request.method,http.route",
    )?;
    let queries = fetch_span_groups(
        args,
        &format!("{service_term} has(db.system.name)"),
        "db.system.name,db.namespace,db.query.text",
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
            if !routes.groups.is_empty() {
                println!();
                print_span_group_table(&["ROUTE"], &routes.groups, |group| {
                    vec![format_route_label(group)]
                })?;
            }
            if !queries.groups.is_empty() {
                println!();
                print_span_group_table(&["DATABASE", "QUERY"], &queries.groups, |group| {
                    vec![format_database_label(group), format_query_label(group)]
                })?;
            }
            if args.buckets {
                println!();
                println!("every {}", table::format_duration(service.step_ns));
                print_service_steps(&service.buckets)?;
            }
        }
        OutputFormat::Json if args.buckets => print_json(&ServiceReport {
            service: &service,
            routes: &routes.groups,
            queries: &queries.groups,
        })?,
        OutputFormat::Json => print_json(&ServiceReport {
            service: ServiceWithoutBuckets::from(&service),
            routes: &routes.groups,
            queries: &queries.groups,
        })?,
    }
    note_truncation(
        routes.truncated || queries.truncated,
        "The service has more routes or queries; raise --limit.",
    );
    Ok(())
}

fn fetch_span_groups(args: &ServiceArgs, query: &str, by: &str) -> anyhow::Result<SpanGroups> {
    let mut params = args.range.to_query_params();
    params.push(("q", Some(query.to_owned())));
    params.push(("by", Some(by.to_owned())));
    args.client.get("/api/spans/groups", &params)
}

fn format_route_label(group: &SpanGroup) -> String {
    let method = format_group_value(group, "http.request.method").unwrap_or_default();
    let route = format_group_value(group, "http.route").unwrap_or_else(|| "-".into());
    format!("{method} {route}")
}

fn format_database_label(group: &SpanGroup) -> String {
    let system = format_group_value(group, "db.system.name").unwrap_or_default();
    match format_group_value(group, "db.namespace") {
        Some(namespace) => format!("{system} {namespace}"),
        None => system,
    }
}

fn format_query_label(group: &SpanGroup) -> String {
    format_group_value(group, "db.query.text").unwrap_or_else(|| group.name.clone())
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
        cells.extend(format_span_stats_cells(&bucket.requests));
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
struct ServiceReport<'a, ServiceFields> {
    #[serde(flatten)]
    service: ServiceFields,
    routes: &'a [SpanGroup],
    queries: &'a [SpanGroup],
}

#[derive(Serialize)]
struct ServiceWithoutBuckets<'a> {
    service: &'a str,
    resource: &'a Attributes,
    start_at: Timestamp,
    end_at: Timestamp,
    stats: &'a ServiceStats,
}

impl<'a> From<&'a Service> for ServiceWithoutBuckets<'a> {
    fn from(service: &'a Service) -> Self {
        Self {
            service: &service.service,
            resource: &service.resource,
            start_at: service.start_at,
            end_at: service.end_at,
            stats: &service.stats,
        }
    }
}
