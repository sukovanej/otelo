use clap::{Parser, Subcommand};
use otelo::{cli, init, reindex, serve};

#[derive(Parser)]
#[command(version, about = "Deploy, run, and observe the apps on one server")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Make the password of a data directory, and print it once
    Init(init::InitArgs),
    /// Run the daemon
    Serve(serve::ServeArgs),
    /// Build telemetry.sqlite again from the journal, with the daemon stopped
    Reindex(reindex::ReindexArgs),
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
        Command::Init(args) => init::init_data_directory(&args),
        Command::Serve(args) => serve::run_daemon(args),
        Command::Reindex(args) => reindex::reindex_telemetry(&args),
        Command::Logs(args) => cli::print_logs(&args),
        Command::Spans(args) => cli::print_spans(&args),
        Command::Traces(args) => cli::print_traces(&args),
        Command::Trace(args) => cli::print_trace(&args),
        Command::Metrics(args) => cli::print_metrics(&args),
        Command::Metric(args) => cli::print_metric_series(&args),
        Command::Services(args) => cli::print_services(&args),
        Command::Service(args) => cli::print_service(&args),
        Command::Calls(args) => cli::print_calls(&args),
        Command::Attributes(args) => cli::print_attributes(&args),
        Command::Complete(args) => cli::print_completions(&args),
        Command::Index(args) => cli::change_and_print_indexes(&args),
        Command::Openapi => {
            println!("{}", otelo_api::build_openapi_spec().to_pretty_json()?);
            Ok(())
        }
    }
}
