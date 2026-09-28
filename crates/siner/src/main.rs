mod api;
mod client;
mod query;
mod serve;
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
    /// List traces by their root span, newest first
    Traces(query::TracesArgs),
    /// Print the span tree and the logs of one trace
    Trace(query::TraceArgs),
    /// List the metric series, or print one metric in buckets
    Metrics(query::MetricsArgs),
    /// Run a read-only SQL query over the telemetry
    Sql(query::SqlArgs),
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Serve(args) => serve::main(args),
        Command::Logs(args) => query::logs(&args),
        Command::Traces(args) => query::traces(&args),
        Command::Trace(args) => query::trace(&args),
        Command::Metrics(args) => query::metrics(&args),
        Command::Sql(args) => query::sql(&args),
    }
}
