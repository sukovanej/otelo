mod api;
mod client;
mod query;
mod serve;
mod state;
mod table;

use clap::{Parser, Subcommand};

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
    Logs(query::LogsArgs),
    /// List spans, newest first
    Spans(query::SpansArgs),
    /// List traces by their root span, newest first
    Traces(query::TracesArgs),
    /// Print the span tree and the logs of one trace
    Trace(query::TraceArgs),
    /// List the metric series
    Metrics(query::MetricsArgs),
    /// Print the series of one metric in buckets
    Metric(query::MetricArgs),
    /// Run a read-only SQL query over the telemetry
    Sql(query::SqlArgs),
    /// List the attributes a query can read, with their types
    Attributes(query::AttributesArgs),
    /// Suggest what can go at the cursor of a query
    Complete(query::CompleteArgs),
    /// List, add, or remove the indexed attributes
    Index(query::IndexArgs),
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Serve(args) => serve::main(args),
        Command::Logs(args) => query::logs(&args),
        Command::Spans(args) => query::spans(&args),
        Command::Traces(args) => query::traces(&args),
        Command::Trace(args) => query::trace(&args),
        Command::Metrics(args) => query::metrics(&args),
        Command::Metric(args) => query::metric(&args),
        Command::Sql(args) => query::sql(&args),
        Command::Attributes(args) => query::attributes(&args),
        Command::Complete(args) => query::complete(&args),
        Command::Index(args) => query::index(&args),
    }
}
