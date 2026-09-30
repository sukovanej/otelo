use std::io;
use std::num::NonZeroUsize;

use otelo_storage::MetricKind;
use otelo_storage::query::{
    Bucket, BucketChange, GroupKey, MetricList, MetricSeries, Resolution, SeriesGroup,
};

use super::client::{Client, OutputFormat, escape_path_segment, note_truncation, print_json};
use super::table::{self, Table};
use super::{RangeArgs, join_query_words};

#[derive(clap::Args)]
pub struct MetricsArgs {
    /// The series to keep, by name, service, kind, unit, labels, and
    /// resource, such as 'name ~ http service = api'. Quote it for the shell
    query: Vec<String>,

    #[command(flatten)]
    range: RangeArgs,

    #[command(flatten)]
    client: Client,
}

pub fn print_metrics(args: &MetricsArgs) -> anyhow::Result<()> {
    let mut params = args.range.to_query_params();
    params.push(("q", join_query_words(&args.query)));
    let answer: MetricList = args.client.get("/api/metrics", &params)?;
    match args.client.choose_output_format() {
        OutputFormat::Table => {
            let mut table = Table::new(&["NAME", "KIND", "UNIT", "SERVICE", "LABELS"]);
            for series in &answer.series {
                table.add_row(vec![
                    series.name.clone(),
                    series.kind.name().into(),
                    series.unit.clone(),
                    series.service.clone(),
                    table::format_labels(&series.labels),
                ]);
            }
            table.print()?;
        }
        OutputFormat::Json => print_json(&answer)?,
    }
    note_truncation(
        answer.truncated,
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

    /// Combine the series with the same values of these names into one
    /// group: labels, service, or resource.<key>. Repeat it, or separate the
    /// names with commas
    #[arg(long, value_delimiter = ',')]
    by: Vec<String>,

    /// Keep the N groups with the highest value over the range, and combine
    /// the rest into one group other
    #[arg(long)]
    top: Option<NonZeroUsize>,

    #[command(flatten)]
    range: RangeArgs,

    #[command(flatten)]
    client: Client,
}

pub fn print_metric_series(args: &MetricArgs) -> anyhow::Result<()> {
    let mut params = args.range.to_query_params();
    params.extend([
        ("q", join_query_words(&args.query)),
        ("step", args.step.clone()),
        (
            "resolution",
            args.resolution
                .map(|resolution| resolution.name().to_owned()),
        ),
        ("by", (!args.by.is_empty()).then(|| args.by.join(","))),
        ("top", args.top.map(|top| top.to_string())),
    ]);
    let metric: MetricSeries = args.client.get(
        &format!("/api/metrics/{}", escape_path_segment(&args.name)),
        &params,
    )?;
    match args.client.choose_output_format() {
        OutputFormat::Table => {
            for (position, group) in metric.groups.iter().enumerate() {
                if position > 0 {
                    println!();
                }
                println!(
                    "{} {} every {}{}",
                    metric.name,
                    describe_group(group),
                    table::format_duration(metric.step_ns),
                    match metric.resolution {
                        Resolution::Raw => String::new(),
                        summaries => format!(" of {} summaries", summaries.name()),
                    },
                );
                match group.kind {
                    MetricKind::Gauge | MetricKind::UpDown => print_levels(&group.buckets)?,
                    MetricKind::Counter(_) => print_rates(&group.buckets)?,
                    MetricKind::Histogram(_) => print_distributions(&group.buckets)?,
                }
            }
        }
        OutputFormat::Json => print_json(&metric)?,
    }
    note_truncation(
        metric.truncated,
        "More series or groups match; narrow them with the query, combine them with --by or \
         --top, or raise --limit.",
    );
    Ok(())
}

fn describe_group(group: &SeriesGroup) -> String {
    let kind_and_unit = format!("{} {}", group.kind.name(), group.unit);
    match &group.key {
        GroupKey::Series {
            service, labels, ..
        } => {
            let labels = table::format_labels(labels);
            if labels.is_empty() {
                format!("{service} {kind_and_unit}")
            } else {
                format!("{service} {kind_and_unit} {labels}")
            }
        }
        GroupKey::Values {
            values,
            series_count,
        } => {
            // The series of this group have none of the names of --by.
            let values = if values.is_empty() {
                "-".to_owned()
            } else {
                table::format_labels(values)
            };
            format!("{kind_and_unit} {values} ({series_count} series)")
        }
        GroupKey::Other {
            group_count,
            series_count,
        } => {
            let groups = if *group_count == 1 { "group" } else { "groups" };
            format!("{kind_and_unit} other ({group_count} {groups}, {series_count} series)")
        }
    }
}

fn print_levels(buckets: &[Bucket]) -> io::Result<()> {
    let mut table = Table::new(&["TIME (UTC)", "COUNT", "MIN", "AVG", "MAX", "LAST"]);
    for bucket in buckets {
        table.add_row(vec![
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
        table.add_row(vec![
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
    let format_estimate =
        |value: Option<f64>| value.map_or_else(|| "-".into(), table::format_number);
    for bucket in buckets {
        // The first step of a cumulative histogram has no distribution.
        let BucketChange::Distribution(distribution) = &bucket.change else {
            let mut cells = vec!["-".to_owned(); 6];
            cells[0] = table::format_utc_time(bucket.start_at);
            table.add_row(cells);
            continue;
        };
        #[expect(clippy::cast_precision_loss, reason = "an average to print")]
        let avg = distribution
            .sum
            .filter(|_| distribution.count > 0)
            .map(|sum| sum / distribution.count as f64);
        let percentiles = distribution.percentiles;
        table.add_row(vec![
            table::format_utc_time(bucket.start_at),
            distribution.count.to_string(),
            format_estimate(avg),
            format_estimate(percentiles.map(|percentiles| percentiles.p50)),
            format_estimate(percentiles.map(|percentiles| percentiles.p90)),
            format_estimate(percentiles.map(|percentiles| percentiles.p99)),
        ]);
    }
    table.print()
}
