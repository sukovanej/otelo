use clap::{Parser, Subcommand};
use siner::{cli, serve};

#[derive(Parser)]
#[command(version, about = "Deploy, run, and observe the apps on one server")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the daemon
    Serve(serve::Args),
    /// Print log lines, grouped by message template unless --raw
    Logs(cli::LogsArgs),
    /// List spans, newest first
    Spans(cli::SpansArgs),
    /// List traces by their root span, newest first
    Traces(cli::TracesArgs),
    /// Print the span tree and the logs of one trace
    Trace(cli::TraceArgs),
    /// List the metric series
    Metrics(cli::MetricsArgs),
    /// Print the series of one metric in buckets
    Metric(cli::MetricArgs),
    /// List the services with their requests, errors, latency, and logs
    Services(cli::ServicesArgs),
    /// Print the requests, errors, latency, and logs of one service, by operation
    Service(cli::ServiceArgs),
    /// Print the calls one service makes, by database, host, or other target, and by operation
    Calls(cli::CallsArgs),
    /// Run a read-only SQL query over the telemetry
    Sql(cli::SqlArgs),
    /// List the attributes a query can read, with their types
    Attributes(cli::AttributesArgs),
    /// Suggest what can go at the cursor of a query
    Complete(cli::CompleteArgs),
    /// List, add, or remove the indexed attributes
    Index(cli::IndexArgs),
    /// Print the spec of the query API that /api/openapi.json serves, without a daemon
    Openapi,
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Serve(args) => serve::main(args),
        Command::Logs(args) => cli::logs(&args),
        Command::Spans(args) => cli::spans(&args),
        Command::Traces(args) => cli::traces(&args),
        Command::Trace(args) => cli::trace(&args),
        Command::Metrics(args) => cli::metrics(&args),
        Command::Metric(args) => cli::metric(&args),
        Command::Services(args) => cli::services(&args),
        Command::Service(args) => cli::service(&args),
        Command::Calls(args) => cli::calls(&args),
        Command::Sql(args) => cli::sql(&args),
        Command::Attributes(args) => cli::attributes(&args),
        Command::Complete(args) => cli::complete(&args),
        Command::Index(args) => cli::index(&args),
        Command::Openapi => {
            println!("{}", siner_api::spec().to_pretty_json()?);
            Ok(())
        }
    }
}
