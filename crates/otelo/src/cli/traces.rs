use std::collections::HashMap;

use otelo_indexed_storage::SpanId;
use otelo_indexed_storage::query::{
    RankOrder, SpanGroupRank, SpanGroupingField, SpanGroups, SpanSort, Spans, Trace, TraceSpan,
    Traces,
};
use otelo_query::Signal;

use super::client::{Client, OutputFormat, escape_path_segment, note_truncation, print_json};
use super::span_groups::{format_group_value, print_span_group_table};
use super::table::{self, Table};
use super::{QUERY_HELP, RangeArgs, join_query_words, note_unindexed_keys};

const RANK_HELP: &str = "With --by, the number the groups rank by: time, count, errors, \
    error_rate, p50, p95, or p99 [default: time]";

#[derive(clap::Args)]
pub struct SpansArgs {
    #[arg(help = QUERY_HELP)]
    query: Vec<String>,

    /// Print the spans grouped by these names, separated by commas, with the
    /// count, errors, latency, and total time of each group: attributes,
    /// service, name, or resource.<key>, such as http.request.method,http.route
    #[arg(long)]
    by: Option<String>,

    /// The order of the spans: newest or oldest by start, longest or shortest
    /// by duration [default: newest]
    #[arg(long, conflicts_with = "by")]
    sort: Option<SpanSort>,

    #[arg(long, requires = "by", help = RANK_HELP)]
    rank: Option<SpanGroupRank>,

    /// With --by, whether the groups with the highest or the lowest number of
    /// --rank come first and stay within the limit [default: highest]
    #[arg(long, requires = "by")]
    order: Option<RankOrder>,

    #[command(flatten)]
    range: RangeArgs,

    #[command(flatten)]
    client: Client,
}

pub fn print_spans(args: &SpansArgs) -> anyhow::Result<()> {
    if let Some(by) = &args.by {
        return print_span_groups(args, by);
    }
    let params = [
        ("q", join_query_words(&args.query)),
        ("sort", args.sort.map(|sort| sort.name().to_owned())),
    ];
    let answer: Spans = args
        .range
        .fetch_pages(&args.client, "/api/spans", &params)?;
    match args.client.choose_output_format() {
        OutputFormat::Table => {
            let mut table = Table::new(&[
                "TRACE",
                "START (UTC)",
                "SERVICE",
                "DURATION",
                "ERROR",
                "NAME",
            ]);
            for span in &answer.spans {
                table.add_row(vec![
                    span.trace_id.to_string(),
                    table::format_utc_time(span.started_at),
                    span.service.clone(),
                    table::format_duration(span.duration_ns),
                    if span.status.is_error() { "ERROR" } else { "" }.into(),
                    span.name.clone(),
                ]);
            }
            table.print()?;
        }
        OutputFormat::Json => print_json(&answer)?,
    }
    note_truncation(
        answer.next.is_some(),
        "More spans match; narrow them with the query or --since, or raise --limit.",
    );
    note_unindexed_keys(Signal::Spans, &answer.unindexed);
    Ok(())
}

fn print_span_groups(args: &SpansArgs, by: &str) -> anyhow::Result<()> {
    let fields = by
        .split(',')
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .map(|field| field.parse().map_err(anyhow::Error::msg))
        .collect::<anyhow::Result<Vec<SpanGroupingField>>>()?;
    let mut params = args.range.to_query_params();
    params.push(("q", join_query_words(&args.query)));
    params.push(("by", Some(by.to_owned())));
    params.push(("rank", args.rank.map(|rank| rank.name().to_owned())));
    params.push(("order", args.order.map(|order| order.name().to_owned())));
    let answer: SpanGroups = args.client.get("/api/spans/groups", &params)?;
    match args.client.choose_output_format() {
        OutputFormat::Table => {
            let fields: Vec<String> = fields.iter().map(ToString::to_string).collect();
            let labels: Vec<String> = fields.iter().map(|field| field.to_uppercase()).collect();
            let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
            print_span_group_table(&labels, &answer.groups, |group| {
                fields
                    .iter()
                    .map(|field| format_group_value(group, field).unwrap_or_else(|| "-".into()))
                    .collect()
            })?;
        }
        OutputFormat::Json => print_json(&answer)?,
    }
    note_truncation(
        answer.truncated,
        "More groups match; narrow them with the query or --since, or raise --limit.",
    );
    note_unindexed_keys(Signal::Spans, &answer.unindexed);
    Ok(())
}

#[derive(clap::Args)]
pub struct TracesArgs {
    /// The traces to keep: those with a span that the query keeps, such as
    /// 'error = true' or 'root = true AND duration > 500ms'. Quote it for the
    /// shell
    query: Vec<String>,

    /// The order of the traces by their root span: newest or oldest by start,
    /// longest or shortest by duration [default: newest]
    #[arg(long)]
    sort: Option<SpanSort>,

    #[command(flatten)]
    range: RangeArgs,

    #[command(flatten)]
    client: Client,
}

pub fn print_traces(args: &TracesArgs) -> anyhow::Result<()> {
    let params = [
        ("q", join_query_words(&args.query)),
        ("sort", args.sort.map(|sort| sort.name().to_owned())),
    ];
    let answer: Traces = args
        .range
        .fetch_pages(&args.client, "/api/traces", &params)?;
    match args.client.choose_output_format() {
        OutputFormat::Table => {
            let mut table = Table::new(&[
                "TRACE",
                "START (UTC)",
                "SERVICE",
                "DURATION",
                "SPANS",
                "ERROR",
                "NAME",
            ]);
            for trace in &answer.traces {
                table.add_row(vec![
                    trace.trace_id.to_string(),
                    table::format_utc_time(trace.started_at),
                    trace.service.clone(),
                    table::format_duration(trace.duration_ns),
                    trace.spans.to_string(),
                    if trace.error { "ERROR" } else { "" }.into(),
                    trace.name.clone(),
                ]);
            }
            table.print()?;
        }
        OutputFormat::Json => print_json(&answer)?,
    }
    note_truncation(
        answer.next.is_some(),
        "More traces match; narrow them with the query or --since, or raise --limit.",
    );
    note_unindexed_keys(Signal::Spans, &answer.unindexed);
    Ok(())
}

#[derive(clap::Args)]
pub struct TraceArgs {
    /// The trace ID, 32 hex digits
    #[arg(value_name = "ID")]
    trace_id: String,

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

pub fn print_trace(args: &TraceArgs) -> anyhow::Result<()> {
    let params = [
        ("since", args.since.clone()),
        ("until", args.until.clone()),
        ("limit", args.limit.map(|limit| limit.to_string())),
    ];
    let trace: Trace = args.client.get(
        &format!("/api/traces/{}", escape_path_segment(&args.trace_id)),
        &params,
    )?;
    match args.client.choose_output_format() {
        OutputFormat::Table => print_span_tree_and_logs(&trace)?,
        OutputFormat::Json => print_json(&trace)?,
    }
    note_truncation(
        trace.truncated,
        "The trace has more spans or logs; raise --limit.",
    );
    Ok(())
}

fn print_span_tree_and_logs(trace: &Trace) -> anyhow::Result<()> {
    let trace_started_at = trace.spans.iter().map(|span| span.started_at).min();
    let spans_by_id: HashMap<SpanId, &TraceSpan> = trace
        .spans
        .iter()
        .map(|span| (span.span_id, span))
        .collect();
    let mut children: HashMap<Option<SpanId>, Vec<&TraceSpan>> = HashMap::new();
    for span in &trace.spans {
        let parent_in_trace = span
            .parent_span_id
            .filter(|id| spans_by_id.contains_key(id));
        children.entry(parent_in_trace).or_default().push(span);
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
        let offset_from_trace_start_ns = trace_started_at.map_or(0, |trace_started_at| {
            i64::try_from(span.started_at.duration_since(trace_started_at).as_nanos())
                .unwrap_or(i64::MAX)
        });
        table.add_row(vec![
            format!("{prefix}{}", span.name),
            span.service.clone(),
            format!("+{}", table::format_duration(offset_from_trace_start_ns)),
            table::format_duration(span.duration_ns),
            if span.status.is_error() { "ERROR" } else { "" }.into(),
        ]);
        let child_spans = children.get(&Some(span.span_id));
        let child_spans = child_spans.map_or(&[][..], Vec::as_slice);
        for (position, child) in child_spans.iter().enumerate().rev() {
            let is_last_child = position + 1 == child_spans.len();
            let branch = if is_last_child { "└─ " } else { "├─ " };
            let indent_under = if is_last_child { "   " } else { "│  " };
            stack.push((
                child,
                format!("{indent}{branch}"),
                format!("{indent}{indent_under}"),
            ));
        }
    }
    table.print()?;
    if !trace.logs.is_empty() {
        println!();
        let mut log_table = Table::new(&["TIME (UTC)", "SERVICE", "LEVEL", "BODY"]);
        for line in trace.logs.iter().rev() {
            log_table.add_row(vec![
                table::format_utc_time(line.logged_at),
                line.service.clone(),
                line.severity.level().into(),
                line.body.clone(),
            ]);
        }
        log_table.print()?;
    }
    Ok(())
}
