//! The CLI commands that read telemetry: `logs`, `traces`, `trace`,
//! `metrics`, and `sql`.

use std::collections::HashMap;
use std::io::{self, Read};

use siner_telemetry::query::{
    GROUP_SCAN_LIMIT, LogGroups, Logs, MetricList, MetricSeries, SqlResult, Trace, TraceSpan,
    Traces,
};

use crate::api::SqlRequest;
use crate::client::{Client, note_cut, path_segment, print_json};
use crate::table::{self, Table};

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

#[derive(clap::Args)]
pub struct LogsArgs {
    /// Print the lines, not the groups by message template
    #[arg(long)]
    raw: bool,

    /// Only the lines of this service
    #[arg(long)]
    service: Option<String>,

    /// The lowest severity: trace, debug, info, warn, error, or fatal
    #[arg(long)]
    severity: Option<String>,

    /// Words that every line contains
    #[arg(long)]
    search: Option<String>,

    /// Only the lines of this trace
    #[arg(long)]
    trace: Option<String>,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

pub fn logs(args: &LogsArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.extend([
        ("service", args.service.clone()),
        ("severity", args.severity.clone()),
        ("search", args.search.clone()),
        ("trace_id", args.trace.clone()),
    ]);
    let narrow = "narrow them with --since, --service, --severity, or --search, or raise --limit";
    if args.raw {
        let logs: Logs = args.client.get("/api/logs", &params)?;
        if args.client.wants_table() {
            let mut table = Table::new(&["TIME (UTC)", "SERVICE", "LEVEL", "TRACE", "BODY"]);
            for line in &logs.logs {
                table.row(vec![
                    table::time(line.time),
                    line.service.clone(),
                    line.level.clone(),
                    line.trace_id.clone().unwrap_or_else(|| "-".into()),
                    line.body.clone(),
                ]);
            }
            table.print()?;
        } else {
            print_json(&logs)?;
        }
        note_cut(logs.truncated, &format!("More lines match; {narrow}."));
        return Ok(());
    }
    let groups: LogGroups = args.client.get("/api/logs/groups", &params)?;
    if args.client.wants_table() {
        let mut table = Table::new(&["COUNT", "LEVEL", "SERVICE", "LAST (UTC)", "TEMPLATE"]);
        for group in &groups.groups {
            table.row(vec![
                group.count.to_string(),
                group.level.clone(),
                group.services.join(","),
                table::time(group.last),
                group.template.clone(),
            ]);
            if let Some(sample) = group.samples.first().filter(|s| **s != group.template) {
                table.under(&format!("e.g. {sample}"));
            }
        }
        table.print()?;
    } else {
        print_json(&groups)?;
    }
    note_cut(groups.truncated, &format!("More groups exist; {narrow}."));
    note_cut(
        groups.partial,
        &format!(
            "The groups count only the newest {GROUP_SCAN_LIMIT} lines; narrow the range with --since."
        ),
    );
    Ok(())
}

#[derive(clap::Args)]
pub struct TracesArgs {
    /// Only the traces whose root span is of this service
    #[arg(long)]
    service: Option<String>,

    /// Only the traces whose root span name contains this text
    #[arg(long)]
    name: Option<String>,

    /// Only the traces that take at least this long, such as 500ms
    #[arg(long)]
    min_duration: Option<String>,

    /// Only the traces with a failed span
    #[arg(long)]
    errors: bool,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

pub fn traces(args: &TracesArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.extend([
        ("service", args.service.clone()),
        ("name", args.name.clone()),
        ("min_duration", args.min_duration.clone()),
        ("errors", args.errors.then(|| "true".into())),
    ]);
    let traces: Traces = args.client.get("/api/traces", &params)?;
    if args.client.wants_table() {
        let mut table = Table::new(&[
            "TRACE",
            "START (UTC)",
            "SERVICE",
            "DURATION",
            "SPANS",
            "ERROR",
            "NAME",
        ]);
        for trace in &traces.traces {
            table.row(vec![
                trace.trace_id.clone(),
                table::time(trace.time),
                trace.service.clone(),
                table::duration(trace.duration_ns),
                trace.spans.to_string(),
                if trace.error { "ERROR" } else { "" }.into(),
                trace.name.clone(),
            ]);
        }
        table.print()?;
    } else {
        print_json(&traces)?;
    }
    note_cut(
        traces.truncated,
        "More traces match; narrow them with --since, --service, --name, --min-duration, or \
         --errors, or raise --limit.",
    );
    Ok(())
}

#[derive(clap::Args)]
pub struct TraceArgs {
    /// The trace ID, 32 hex digits
    id: String,

    /// Start of the range: a duration before now or an RFC 3339 timestamp
    /// [default: the whole retention]
    #[arg(long)]
    since: Option<String>,

    /// End of the range, in the form of --since [default: now]
    #[arg(long)]
    until: Option<String>,

    /// The most spans, and the most logs, to print
    #[arg(long)]
    limit: Option<usize>,

    #[command(flatten)]
    client: Client,
}

pub fn trace(args: &TraceArgs) -> anyhow::Result<()> {
    let params = [
        ("since", args.since.clone()),
        ("until", args.until.clone()),
        ("limit", args.limit.map(|n| n.to_string())),
    ];
    let trace: Trace = args
        .client
        .get(&format!("/api/traces/{}", path_segment(&args.id)), &params)?;
    if args.client.wants_table() {
        print_tree(&trace)?;
    } else {
        print_json(&trace)?;
    }
    note_cut(
        trace.truncated,
        "The trace has more spans or logs; raise --limit.",
    );
    Ok(())
}

/// The spans as a tree under their parents, then the logs of the trace.
fn print_tree(trace: &Trace) -> anyhow::Result<()> {
    let start = trace.spans.iter().map(|s| s.time).min();
    let ids: HashMap<&str, &TraceSpan> = trace
        .spans
        .iter()
        .map(|s| (s.span_id.as_str(), s))
        .collect();
    let mut children: HashMap<Option<&str>, Vec<&TraceSpan>> = HashMap::new();
    for span in &trace.spans {
        // A span whose parent is missing shows as a root.
        let parent = span
            .parent_span_id
            .as_deref()
            .filter(|id| ids.contains_key(id));
        children.entry(parent).or_default().push(span);
    }
    let mut table = Table::new(&["SPAN", "SERVICE", "START", "DURATION", "ERROR"]);
    let mut stack: Vec<(&TraceSpan, String, String)> = children
        .get(&None)
        .into_iter()
        .flatten()
        .rev()
        .map(|&span| (span, String::new(), String::new()))
        .collect();
    while let Some((span, prefix, indent)) = stack.pop() {
        let offset = start.map_or(0, |start| {
            i64::try_from(span.time.duration_since(start).as_nanos()).unwrap_or(i64::MAX)
        });
        table.row(vec![
            format!("{prefix}{}", span.name),
            span.service.clone(),
            format!("+{}", table::duration(offset)),
            table::duration(span.duration_ns),
            if span.error { "ERROR" } else { "" }.into(),
        ]);
        let kids = children.get(&Some(span.span_id.as_str()));
        let kids = kids.map_or(&[][..], Vec::as_slice);
        for (i, kid) in kids.iter().enumerate().rev() {
            let last = i + 1 == kids.len();
            let branch = if last { "└─ " } else { "├─ " };
            let under = if last { "   " } else { "│  " };
            stack.push((kid, format!("{indent}{branch}"), format!("{indent}{under}")));
        }
    }
    table.print()?;
    if !trace.logs.is_empty() {
        println!();
        let mut logs = Table::new(&["TIME (UTC)", "SERVICE", "LEVEL", "BODY"]);
        for line in trace.logs.iter().rev() {
            logs.row(vec![
                table::time(line.time),
                line.service.clone(),
                line.level.clone(),
                line.body.clone(),
            ]);
        }
        logs.print()?;
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct MetricsArgs {
    /// The metric to print; without it, the list of series
    name: Option<String>,

    /// Only the series of this service
    #[arg(long)]
    service: Option<String>,

    /// Only the series with this label, as name=value; repeat for more
    #[arg(long = "label", value_name = "NAME=VALUE")]
    labels: Vec<String>,

    /// The length of a bucket, such as 1m [default: one that makes 120 buckets
    /// at most]
    #[arg(long)]
    step: Option<String>,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

pub fn metrics(args: &MetricsArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.push(("service", args.service.clone()));
    let Some(name) = &args.name else {
        let list: MetricList = args.client.get("/api/metrics", &params)?;
        if args.client.wants_table() {
            let mut table = Table::new(&["NAME", "KIND", "UNIT", "SERVICE", "LABELS"]);
            for series in &list.series {
                table.row(vec![
                    series.name.clone(),
                    series.kind.clone(),
                    series.unit.clone(),
                    series.service.clone(),
                    table::labels(&series.labels),
                ]);
            }
            table.print()?;
        } else {
            print_json(&list)?;
        }
        note_cut(
            list.truncated,
            "More series exist; narrow them with --service, or raise --limit.",
        );
        return Ok(());
    };
    params.extend([
        (
            "labels",
            (!args.labels.is_empty()).then(|| args.labels.join(",")),
        ),
        ("step", args.step.clone()),
    ]);
    let metric: MetricSeries = args
        .client
        .get(&format!("/api/metrics/{}", path_segment(name)), &params)?;
    if args.client.wants_table() {
        for (i, series) in metric.series.iter().enumerate() {
            if i > 0 {
                println!();
            }
            let labels = table::labels(&series.labels);
            println!(
                "{} {} {} {}{} every {}",
                metric.name,
                series.service,
                series.kind,
                series.unit,
                if labels.is_empty() {
                    String::new()
                } else {
                    format!(" {labels}")
                },
                table::duration(metric.step_ns),
            );
            let mut table = Table::new(&["TIME (UTC)", "COUNT", "MIN", "AVG", "MAX", "LAST"]);
            for bucket in &series.buckets {
                table.row(vec![
                    table::time(bucket.time),
                    bucket.count.to_string(),
                    table::number(bucket.min),
                    table::number(bucket.avg),
                    table::number(bucket.max),
                    table::number(bucket.last),
                ]);
            }
            table.print()?;
        }
    } else {
        print_json(&metric)?;
    }
    note_cut(
        metric.truncated,
        "More series match; narrow them with --service or --label, or raise --limit.",
    );
    Ok(())
}

#[derive(clap::Args)]
pub struct SqlArgs {
    /// One SELECT, or - to read it from stdin. It reads the views resources,
    /// logs, spans, series, and points, which join the day files of the range
    /// with a day column in front
    query: String,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

pub fn sql(args: &SqlArgs) -> anyhow::Result<()> {
    let sql = if args.query == "-" {
        let mut text = String::new();
        io::stdin().read_to_string(&mut text)?;
        text
    } else {
        args.query.clone()
    };
    let request = SqlRequest {
        sql,
        since: args.range.since.clone(),
        until: args.range.until.clone(),
        limit: args.range.limit,
    };
    let result: SqlResult = args.client.post("/api/sql", &request)?;
    if args.client.wants_table() {
        let columns: Vec<&str> = result.columns.iter().map(String::as_str).collect();
        let mut table = Table::new(&columns);
        for row in &result.rows {
            table.row(row.iter().map(|value| match value {
                serde_json::Value::Null => String::new(),
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            }));
        }
        table.print()?;
    } else {
        print_json(&result)?;
    }
    note_cut(
        result.truncated,
        "The query returned more rows; narrow it with a WHERE, or raise --limit.",
    );
    Ok(())
}
