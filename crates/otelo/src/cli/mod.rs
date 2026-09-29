mod catalog;
pub mod client;
mod logs;
mod metrics;
mod services;
mod sql;
pub mod table;
mod traces;

use otelo_query::Signal;

pub use catalog::{AttributesArgs, CompleteArgs, IndexArgs, attributes, complete, index};
pub use logs::{LogsArgs, logs};
pub use metrics::{MetricArgs, MetricsArgs, metric, metrics};
pub use services::{CallsArgs, ServiceArgs, ServicesArgs, calls, service, services};
pub use sql::{SqlArgs, sql};
pub use traces::{SpansArgs, TraceArgs, TracesArgs, spans, trace, traces};

const QUERY_HELP: &str = "The records to keep, such as 'http.route = \"/matches\" OR user.id = 7'. \
    Built-in fields come first; resource.<key> reads a resource attribute, and attr.<key> an \
    attribute named like a built-in field. Quote it for the shell";

/// The time range and the row limit of every query.
#[derive(clap::Args)]
pub struct Range {
    /// Start of the range: a duration before now, such as 1h, 30m, or 2d, or an
    /// RFC 3339 timestamp [default: 1h]
    #[arg(long)]
    since: Option<String>,

    /// End of the range, in the form of --since [default: now]
    #[arg(long)]
    until: Option<String>,

    /// The most rows to print
    #[arg(long)]
    limit: Option<usize>,
}

impl Range {
    fn params(&self) -> Vec<(&'static str, Option<String>)> {
        vec![
            ("since", self.since.clone()),
            ("until", self.until.clone()),
            ("limit", self.limit.map(|n| n.to_string())),
        ]
    }
}

fn join_query_words(words: &[String]) -> Option<String> {
    (!words.is_empty()).then(|| words.join(" "))
}

fn note_unindexed(signal: Signal, keys: &[String]) {
    if let Some(first) = keys.first() {
        let verb = if keys.len() == 1 { "has" } else { "have" };
        eprintln!(
            "{} {verb} no index, so the query read every record in the range; \
             `otelo index add {signal} {first}` adds one.",
            keys.join(", ")
        );
    }
}
