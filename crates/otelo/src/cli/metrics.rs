use std::io;

use otelo_storage::MetricKind;
use otelo_storage::query::{Bucket, BucketChange, MetricList, MetricSeries, Resolution};

use super::client::{Client, escape_path_segment, note_cut, print_json};
use super::table::{self, Table};
use super::{Range, join_query_words};

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
    params.push(("q", join_query_words(&args.query)));
    let list: MetricList = args.client.get("/api/metrics", &params)?;
    if args.client.wants_table() {
        let mut table = Table::new(&["NAME", "KIND", "UNIT", "SERVICE", "LABELS"]);
        for series in &list.series {
            table.row(vec![
                series.name.clone(),
                series.kind.name().to_owned(),
                series.unit.clone(),
                series.service.clone(),
                table::format_labels(&series.labels),
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

    /// The points to read: raw, or their summaries by the minute (1m) or by
    /// the hour (1h) [default: the finest that are kept for the range and are
    /// not too many]
    #[arg(long)]
    resolution: Option<Resolution>,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

pub fn metric(args: &MetricArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.extend([
        ("q", join_query_words(&args.query)),
        ("step", args.step.clone()),
        (
            "resolution",
            args.resolution
                .map(|resolution| resolution.name().to_owned()),
        ),
    ]);
    let metric: MetricSeries = args.client.get(
        &format!("/api/metrics/{}", escape_path_segment(&args.name)),
        &params,
    )?;
    if args.client.wants_table() {
        for (i, series) in metric.series.iter().enumerate() {
            if i > 0 {
                println!();
            }
            let labels = table::format_labels(&series.labels);
            println!(
                "{} {} {} {}{} every {}{}",
                metric.name,
                series.service,
                series.kind.name(),
                series.unit,
                if labels.is_empty() {
                    String::new()
                } else {
                    format!(" {labels}")
                },
                table::format_duration(metric.step_ns),
                match metric.resolution {
                    Resolution::Raw => String::new(),
                    summaries => format!(" of {} summaries", summaries.name()),
                },
            );
            match series.kind {
                MetricKind::Gauge | MetricKind::UpDown => print_levels(&series.buckets)?,
                MetricKind::Counter(_) => print_rates(&series.buckets)?,
                MetricKind::Histogram(_) => print_distributions(&series.buckets)?,
            }
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

fn print_levels(buckets: &[Bucket]) -> io::Result<()> {
    let mut table = Table::new(&["TIME (UTC)", "COUNT", "MIN", "AVG", "MAX", "LAST"]);
    for bucket in buckets {
        table.row(vec![
            table::format_utc_time(bucket.start_at),
            bucket.count.to_string(),
            table::format_number(bucket.min),
            table::format_number(bucket.avg),
            table::format_number(bucket.max),
            table::format_number(bucket.last),
        ]);
    }
    table.print()
}

fn print_rates(buckets: &[Bucket]) -> io::Result<()> {
    let mut table = Table::new(&["TIME (UTC)", "COUNT", "PER SECOND", "TOTAL"]);
    for bucket in buckets {
        table.row(vec![
            table::format_utc_time(bucket.start_at),
            bucket.count.to_string(),
            match bucket.change {
                BucketChange::Rate { per_second } => table::format_number(per_second),
                // The first point of a series has nothing to count from.
                BucketChange::None | BucketChange::Distribution(_) => "-".into(),
            },
            table::format_number(bucket.last),
        ]);
    }
    table.print()
}

fn print_distributions(buckets: &[Bucket]) -> io::Result<()> {
    let mut table = Table::new(&["TIME (UTC)", "COUNT", "AVG", "P50", "P90", "P99"]);
    let estimate = |value: Option<f64>| value.map_or_else(|| "-".into(), table::format_number);
    for bucket in buckets {
        // The first step of a cumulative histogram has no distribution.
        let BucketChange::Distribution(distribution) = &bucket.change else {
            let mut cells = vec!["-".to_owned(); 6];
            cells[0] = table::format_utc_time(bucket.start_at);
            table.row(cells);
            continue;
        };
        #[expect(clippy::cast_precision_loss, reason = "an average to print")]
        let avg = distribution
            .sum
            .filter(|_| distribution.count > 0)
            .map(|sum| sum / distribution.count as f64);
        table.row(vec![
            table::format_utc_time(bucket.start_at),
            distribution.count.to_string(),
            estimate(avg),
            estimate(distribution.p50),
            estimate(distribution.p90),
            estimate(distribution.p99),
        ]);
    }
    table.print()
}
