//! `siner spans`, `siner traces`, and `siner trace`.

use std::collections::HashMap;

use siner_query::Signal;
use siner_telemetry::query::{Spans, Trace, TraceSpan, Traces};

use super::client::{Client, note_cut, path_segment, print_json};
use super::table::{self, Table};
use super::{QUERY_HELP, Range, joined, note_unindexed};

#[derive(clap::Args)]
pub struct SpansArgs {
    #[arg(help = QUERY_HELP)]
    query: Vec<String>,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

/// Runs `siner spans`.
///
/// # Errors
///
/// When the daemon cannot be reached, or answers with an error.
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

/// Runs `siner traces`.
///
/// # Errors
///
/// When the daemon cannot be reached, or answers with an error.
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

/// Runs `siner trace`.
///
/// # Errors
///
/// When the daemon cannot be reached, or answers with an error.
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
