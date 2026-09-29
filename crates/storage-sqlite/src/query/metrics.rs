use std::collections::{BTreeMap, btree_map::Entry};

use anyhow::ensure;
use otelo_query::{Query, Signal};
use otelo_storage::query::{
    Bucket, MAX_BUCKETS, MetricFilter, MetricList, MetricSeries, Series, SeriesInfo,
};
use otelo_storage::{Histogram, Merger};

use super::compile::{TableAliases, compile_query};
use super::{WhereClause, timestamp_from_nanos, truncate_to_limit};
use crate::Reader;

// Series from different day files are one series when these match.
type SeriesKey = (String, String, String, String, String);

const ALIASES: TableAliases = TableAliases {
    record: "s",
    resource: "r",
};

struct BucketTally {
    count: u64,
    min: f64,
    max: f64,
    sum: f64,
    last: f64,
}

impl BucketTally {
    const fn start_with_point(value: f64) -> Self {
        Self {
            count: 1,
            min: value,
            max: value,
            sum: value,
            last: value,
        }
    }

    const fn add_point(&mut self, value: f64) {
        self.count += 1;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.sum += value;
        self.last = value;
    }

    fn into_bucket(self, start_at: i64) -> Bucket {
        #[expect(clippy::cast_precision_loss, reason = "a count below 2^53")]
        let count = self.count as f64;
        Bucket {
            start_at: timestamp_from_nanos(start_at),
            count: self.count,
            min: self.min,
            max: self.max,
            avg: self.sum / count,
            last: self.last,
            histogram: None,
        }
    }
}

fn finish_series(
    (service, kind, unit, labels, resource): SeriesKey,
    tallies: BTreeMap<i64, BucketTally>,
    merger: Merger,
) -> anyhow::Result<Series> {
    let mut buckets: BTreeMap<i64, Bucket> = tallies
        .into_iter()
        .map(|(start_at, tally)| (start_at, tally.into_bucket(start_at)))
        .collect();
    for (start, distribution) in merger.finish() {
        if let Some(bucket) = buckets.get_mut(&start) {
            bucket.histogram = Some(distribution);
        }
    }
    Ok(Series {
        service,
        kind,
        unit,
        labels: serde_json::from_str(&labels)?,
        resource: serde_json::from_str(&resource)?,
        buckets: buckets.into_values().collect(),
    })
}

pub(super) fn list_metrics(
    reader: &Reader,
    query: &Query,
    limit: usize,
) -> anyhow::Result<MetricList> {
    ensure!(
        query.signal == Signal::Metrics,
        "the query is over {}, not metrics",
        query.signal
    );
    let mut where_ = WhereClause::new();
    where_.push_clause(
        "EXISTS (SELECT 1 FROM $day.points p
                 WHERE p.series_id = s.id AND p.ts >= :since AND p.ts < :until)"
            .into(),
    );
    where_.push_param(":since", reader.range().start_at());
    where_.push_param(":until", reader.range().end_at());
    compile_query(
        query,
        ALIASES,
        reader.indexed_attributes(),
        "q",
        &mut where_,
    )?;
    let tail = format!(") ORDER BY name, service, labels LIMIT {}", limit + 1);
    let mut series = reader.collect_rows(
        ["SELECT DISTINCT * FROM (", &tail],
        |day| {
            format!(
                "SELECT s.name, s.kind, s.unit, r.service, s.labels,
                        r.attributes AS resource
                 FROM {day}.series s JOIN {day}.resources r ON r.id = s.resource_id
                 WHERE {}",
                where_.sql_for_day(day)
            )
        },
        &where_,
        |row| {
            let labels: String = row.get(4)?;
            let resource: String = row.get(5)?;
            Ok(SeriesInfo {
                name: row.get(0)?,
                kind: row.get(1)?,
                unit: row.get(2)?,
                service: row.get(3)?,
                labels: serde_json::from_str(&labels)?,
                resource: serde_json::from_str(&resource)?,
            })
        },
    )?;
    let truncated = truncate_to_limit(&mut series, limit);
    Ok(MetricList { series, truncated })
}

pub(super) fn read_metric_buckets(
    reader: &Reader,
    filter: &MetricFilter,
    limit: usize,
) -> anyhow::Result<MetricSeries> {
    let step = filter.step_ns;
    ensure!(step > 0, "the step has to be longer than zero");
    ensure!(
        reader.range().length() / step <= MAX_BUCKETS,
        "the step makes more than {MAX_BUCKETS} buckets in the range; raise the step"
    );
    ensure!(
        filter.query.signal == Signal::Metrics,
        "the query is over {}, not metrics",
        filter.query.signal
    );
    let mut where_ = WhereClause::within_reader_range(reader, "p.ts");
    where_.push_clause_with_param("s.name = :name", ":name", filter.name.clone());
    compile_query(
        &filter.query,
        ALIASES,
        reader.indexed_attributes(),
        "q",
        &mut where_,
    )?;
    let mut series: BTreeMap<SeriesKey, (BTreeMap<i64, BucketTally>, Merger)> = BTreeMap::new();
    let mut truncated = false;
    reader.scan_rows(
        ["", " ORDER BY ts"],
        |day| {
            format!(
                "SELECT r.service, s.kind, s.unit, s.labels, r.attributes, p.ts, p.value,
                        p.histogram
                 FROM {day}.points p
                 JOIN {day}.series s ON s.id = p.series_id
                 JOIN {day}.resources r ON r.id = s.resource_id
                 WHERE {}",
                where_.sql_for_day(day)
            )
        },
        &where_,
        |row| {
            let key: SeriesKey = (
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            );
            let recorded_at: i64 = row.get(5)?;
            let value: f64 = row.get(6)?;
            let histogram: Option<String> = row.get(7)?;
            let len = series.len();
            let (buckets, merger) = match series.entry(key) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(_) if len == limit => {
                    truncated = true;
                    return Ok(true);
                }
                Entry::Vacant(entry) => entry.insert((BTreeMap::new(), Merger::default())),
            };
            let start = recorded_at.div_euclid(step) * step;
            if let Some(histogram) = histogram {
                merger.push(start, serde_json::from_str::<Histogram>(&histogram)?);
            }
            match buckets.entry(start) {
                Entry::Occupied(tally) => tally.into_mut().add_point(value),
                Entry::Vacant(slot) => {
                    slot.insert(BucketTally::start_with_point(value));
                }
            }
            Ok(true)
        },
    )?;
    let series = series
        .into_iter()
        .map(|(key, (buckets, merger))| finish_series(key, buckets, merger))
        .collect::<anyhow::Result<_>>()?;
    Ok(MetricSeries {
        name: filter.name.clone(),
        step_ns: step,
        series,
        truncated,
    })
}
