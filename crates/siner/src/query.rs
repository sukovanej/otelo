//! The CLI commands that read telemetry: `logs`, `spans`, `traces`, `trace`,
//! `metrics`, `metric`, `sql`, `attributes`, `complete`, and `index`.

use std::collections::HashMap;
use std::io::{self, Read};

use siner_query::Signal;
use siner_telemetry::query::{
    Attributes, GROUP_SCAN_LIMIT, LogGroups, Logs, MetricList, MetricSeries, Spans, SqlResult,
    Trace, TraceSpan, Traces,
};

use crate::api::{Completions, IndexList, SqlRequest};
use crate::client::{Client, note_cut, path_segment, print_json};
use crate::table::{self, Table};

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

/// A query given as one argument or as several words.
fn joined(words: &[String]) -> Option<String> {
    (!words.is_empty()).then(|| words.join(" "))
}

/// Tells on stderr which attributes of the query have no index.
fn note_unindexed(signal: Signal, keys: &[String]) {
    if let Some(first) = keys.first() {
        let verb = if keys.len() == 1 { "has" } else { "have" };
        eprintln!(
            "{} {verb} no index, so the query read every record in the range; \
             `siner index add {signal} {first}` adds one.",
            keys.join(", ")
        );
    }
}

#[derive(clap::Args)]
pub struct LogsArgs {
    #[arg(help = QUERY_HELP)]
    query: Vec<String>,

    /// Print the lines, not the groups by message template
    #[arg(long)]
    raw: bool,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

pub fn logs(args: &LogsArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.push(("q", joined(&args.query)));
    let narrow = "narrow them with the query or --since, or raise --limit";
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
        note_unindexed(Signal::Logs, &logs.unindexed);
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
            "The groups count only the newest {GROUP_SCAN_LIMIT} lines; narrow the query or --since."
        ),
    );
    note_unindexed(Signal::Logs, &groups.unindexed);
    Ok(())
}

#[derive(clap::Args)]
pub struct SpansArgs {
    #[arg(help = QUERY_HELP)]
    query: Vec<String>,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

pub fn spans(args: &SpansArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.push(("q", joined(&args.query)));
    let spans: Spans = args.client.get("/api/spans", &params)?;
    if args.client.wants_table() {
        let mut table = Table::new(&[
            "TRACE",
            "START (UTC)",
            "SERVICE",
            "DURATION",
            "ERROR",
            "NAME",
        ]);
        for span in &spans.spans {
            table.row(vec![
                span.trace_id.clone(),
                table::time(span.time),
                span.service.clone(),
                table::duration(span.duration_ns),
                if span.error { "ERROR" } else { "" }.into(),
                span.name.clone(),
            ]);
        }
        table.print()?;
    } else {
        print_json(&spans)?;
    }
    note_cut(
        spans.truncated,
        "More spans match; narrow them with the query or --since, or raise --limit.",
    );
    note_unindexed(Signal::Spans, &spans.unindexed);
    Ok(())
}

#[derive(clap::Args)]
pub struct TracesArgs {
    /// The traces to keep: those with a span that the query keeps, such as
    /// 'error = true' or 'root = true AND duration > 500ms'. Quote it for the
    /// shell
    query: Vec<String>,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

pub fn traces(args: &TracesArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.push(("q", joined(&args.query)));
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
        "More traces match; narrow them with the query or --since, or raise --limit.",
    );
    note_unindexed(Signal::Spans, &traces.unindexed);
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
    /// The series to keep, by name, service, kind, unit, labels, and
    /// resource, such as 'name ~ http service = api'. Quote it for the shell
    query: Vec<String>,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

pub fn metrics(args: &MetricsArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.push(("q", joined(&args.query)));
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
        "More series exist; narrow them with the query, or raise --limit.",
    );
    Ok(())
}

#[derive(clap::Args)]
pub struct MetricArgs {
    /// The name of the metric
    name: String,

    /// The series of the metric to keep, by labels and resource, such as
    /// 'state = used'. Quote it for the shell
    query: Vec<String>,

    /// The length of a bucket, such as 1m [default: one that makes 120 buckets
    /// at most]
    #[arg(long)]
    step: Option<String>,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

pub fn metric(args: &MetricArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.extend([("q", joined(&args.query)), ("step", args.step.clone())]);
    let metric: MetricSeries = args.client.get(
        &format!("/api/metrics/{}", path_segment(&args.name)),
        &params,
    )?;
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
        "More series match; narrow them with the query, or raise --limit.",
    );
    Ok(())
}

#[derive(clap::Args)]
pub struct SqlArgs {
    /// One SELECT, or - to read it from stdin. It reads the views resources,
    /// logs, spans, series, and points, which join the day files of the range
    /// with a day column in front, and the attribute catalog
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

#[derive(clap::Args)]
pub struct AttributesArgs {
    /// logs, spans, or metrics
    signal: Signal,

    #[command(flatten)]
    client: Client,
}

pub fn attributes(args: &AttributesArgs) -> anyhow::Result<()> {
    let attributes: Attributes = args.client.get(
        "/api/attributes",
        &[("signal", Some(args.signal.to_string()))],
    )?;
    if args.client.wants_table() {
        let mut table = Table::new(&["FIELD", "TYPE", "COUNT", "INDEXED"]);
        let rows = attributes
            .record
            .iter()
            .map(|a| (siner_query::Field::Attribute(a.key.clone()), a))
            .chain(
                attributes
                    .resource
                    .iter()
                    .map(|a| (siner_query::Field::Resource(a.key.clone()), a)),
            );
        for (field, attribute) in rows {
            table.row(vec![
                field.to_string(),
                attribute.kind.clone(),
                attribute.count.to_string(),
                if attribute.indexed { "yes" } else { "" }.into(),
            ]);
        }
        table.print()?;
    } else {
        print_json(&attributes)?;
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct CompleteArgs {
    /// logs, spans, or metrics
    signal: Signal,

    /// The query as typed so far
    #[arg(default_value = "")]
    query: String,

    /// The position of the cursor in the query, in characters [default: the
    /// end]
    #[arg(long)]
    cursor: Option<usize>,

    #[command(flatten)]
    client: Client,
}

pub fn complete(args: &CompleteArgs) -> anyhow::Result<()> {
    let completions: Completions = args.client.get(
        "/api/complete",
        &[
            ("signal", Some(args.signal.to_string())),
            ("q", Some(args.query.clone())),
            ("cursor", args.cursor.map(|n| n.to_string())),
        ],
    )?;
    if args.client.wants_table() {
        let mut table = Table::new(&["SUGGESTION", "KIND", "DETAIL"]);
        for suggestion in &completions.suggestions {
            table.row(vec![
                suggestion.text.clone(),
                suggestion.kind.clone(),
                suggestion.detail.clone().unwrap_or_default(),
            ]);
        }
        table.print()?;
    } else {
        print_json(&completions)?;
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct IndexArgs {
    #[command(subcommand)]
    command: IndexCommand,

    #[command(flatten)]
    client: Client,
}

#[derive(clap::Subcommand)]
enum IndexCommand {
    /// List the indexed attributes
    List,
    /// Index an attribute of the logs or the spans in every day file
    Add {
        /// logs or spans
        signal: Signal,
        /// The attribute key, such as user.id
        key: String,
    },
    /// Drop the index of an attribute
    Remove {
        /// logs or spans
        signal: Signal,
        /// The attribute key
        key: String,
    },
}

pub fn index(args: &IndexArgs) -> anyhow::Result<()> {
    let path = |signal: &Signal, key: &str| format!("/api/indexes/{signal}/{}", path_segment(key));
    let list: IndexList = match &args.command {
        IndexCommand::List => args.client.get("/api/indexes", &[])?,
        IndexCommand::Add { signal, key } => args.client.put(&path(signal, key))?,
        IndexCommand::Remove { signal, key } => args.client.delete(&path(signal, key))?,
    };
    if args.client.wants_table() {
        let mut table = Table::new(&["SIGNAL", "KEY"]);
        for index in &list.indexes {
            table.row(vec![index.signal.clone(), index.key.clone()]);
        }
        table.print()?;
    } else {
        print_json(&list)?;
    }
    Ok(())
}
