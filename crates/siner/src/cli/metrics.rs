//! `siner metrics` and `siner metric`.

use std::io;

use siner_telemetry::query::{Bucket, MetricList, MetricSeries};

use super::client::{Client, note_cut, path_segment, print_json};
use super::table::{self, Table};
use super::{Range, joined};

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

/// Runs `siner metrics`.
///
/// # Errors
///
/// When the daemon cannot be reached, or answers with an error.
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

/// Runs `siner metric`.
///
/// # Errors
///
/// When the daemon cannot be reached, or answers with an error.
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
            if series.kind == "histogram" {
                print_distributions(&series.buckets)?;
                continue;
            }
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

/// The values a histogram recorded in each step: how many, their average,
/// and the percentile estimates. A step without a distribution, such as the
/// first of a cumulative histogram, shows dashes.
fn print_distributions(buckets: &[Bucket]) -> io::Result<()> {
    let mut table = Table::new(&["TIME (UTC)", "COUNT", "AVG", "P50", "P90", "P99"]);
    let estimate = |value: Option<f64>| value.map_or_else(|| "-".into(), table::number);
    for bucket in buckets {
        let Some(d) = &bucket.histogram else {
            let mut cells = vec!["-".to_owned(); 6];
            cells[0] = table::time(bucket.time);
            table.row(cells);
            continue;
        };
        #[expect(clippy::cast_precision_loss, reason = "an average to print")]
        let avg = d
            .sum
            .filter(|_| d.count > 0)
            .map(|sum| sum / d.count as f64);
        table.row(vec![
            table::time(bucket.time),
            d.count.to_string(),
            estimate(avg),
            estimate(d.p50),
            estimate(d.p90),
            estimate(d.p99),
        ]);
    }
    table.print()
}
