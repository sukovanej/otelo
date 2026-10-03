use std::io;

use otelo_indexed_storage::AttributeValue;
use otelo_indexed_storage::query::{Latency, SpanGroup, SpanStats};

use super::table::{self, Table};

const SPAN_STATS_HEADER: [&str; 6] = ["SPANS", "ERRORS", "P50", "P95", "P99", "TOTAL"];

pub fn format_span_stats_cells(stats: &SpanStats) -> [String; 5] {
    let format_percentile = |pick: fn(&Latency) -> i64| {
        stats.latency.as_ref().map_or_else(
            || "-".into(),
            |latency| table::format_duration(pick(latency)),
        )
    };
    [
        stats.count.to_string(),
        stats.errors.to_string(),
        format_percentile(|latency| latency.p50),
        format_percentile(|latency| latency.p95),
        format_percentile(|latency| latency.p99),
    ]
}

pub fn format_group_value(group: &SpanGroup, field: &str) -> Option<String> {
    group.values.get(field).map(|value| match value {
        AttributeValue::String(text) => text.clone(),
        _ => value.to_string(),
    })
}

pub fn print_span_group_table(
    labels: &[&str],
    groups: &[SpanGroup],
    label_cells: impl Fn(&SpanGroup) -> Vec<String>,
) -> io::Result<()> {
    let header: Vec<&str> = labels.iter().copied().chain(SPAN_STATS_HEADER).collect();
    let mut table = Table::new(&header);
    for group in groups {
        let mut cells = label_cells(group);
        cells.extend(format_span_stats_cells(&group.spans));
        cells.push(table::format_duration(group.spans.total_ns));
        table.add_row(cells);
    }
    table.print()
}
