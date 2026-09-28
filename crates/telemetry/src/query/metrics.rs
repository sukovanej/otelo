use std::collections::{BTreeMap, btree_map::Entry};

use anyhow::ensure;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::ToSchema;

use siner_query::{Query, Signal};

use super::compile::{Aliases, compile};
use super::{Filter, cut, time};
use crate::histogram::Merger;
use crate::{Distribution, Histogram, Reader};

/// The service, kind, unit, labels, and resource attributes of a series.
/// Series from different day files are one series when these match.
type Key = (String, String, String, String, String);

const ALIASES: Aliases = Aliases {
    record: "s",
    resource: "r",
};

/// The most buckets one series can have.
pub const MAX_BUCKETS: i64 = 10_000;

const SECOND: i64 = 1_000_000_000;

/// The steps [`default_step`] picks from.
const STEPS: [i64; 15] = [
    SECOND,
    5 * SECOND,
    10 * SECOND,
    15 * SECOND,
    30 * SECOND,
    60 * SECOND,
    5 * 60 * SECOND,
    10 * 60 * SECOND,
    15 * 60 * SECOND,
    30 * 60 * SECOND,
    3600 * SECOND,
    3 * 3600 * SECOND,
    6 * 3600 * SECOND,
    12 * 3600 * SECOND,
    86_400 * SECOND,
];

/// The smallest round step that splits the range from `since` to `until`
/// into 120 buckets at most.
#[must_use]
pub fn default_step(since: i64, until: i64) -> i64 {
    step_for(since, until, 120)
}

/// The smallest round step that splits the range from `since` to `until`
/// into `buckets` buckets at most.
#[must_use]
pub fn step_for(since: i64, until: i64, buckets: i64) -> i64 {
    let range = until.saturating_sub(since);
    STEPS
        .into_iter()
        .find(|step| range / step <= buckets)
        .unwrap_or(STEPS[STEPS.len() - 1])
}

/// The series in a range, by name.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct MetricList {
    pub series: Vec<SeriesInfo>,
    /// More series exist than the limit let through.
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct SeriesInfo {
    pub name: String,
    /// `gauge`, `sum`, or `histogram`.
    pub kind: String,
    pub unit: String,
    pub service: String,
    #[schema(value_type = HashMap<String, serde_json::Value>)]
    pub labels: Map<String, Value>,
    /// The attributes of the resource that sends the series.
    #[schema(value_type = HashMap<String, serde_json::Value>)]
    pub resource: Map<String, Value>,
}

#[derive(Clone, Debug)]
pub struct MetricFilter {
    pub name: String,
    /// The series of the metric to keep, by their labels and resource.
    pub query: Query,
    /// The length of a bucket in nanoseconds.
    pub step_ns: i64,
}

/// The series of one metric, each in buckets of one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct MetricSeries {
    pub name: String,
    pub step_ns: i64,
    pub series: Vec<Series>,
    /// More series match than the limit let through.
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Series {
    pub service: String,
    pub kind: String,
    pub unit: String,
    #[schema(value_type = HashMap<String, serde_json::Value>)]
    pub labels: Map<String, Value>,
    #[schema(value_type = HashMap<String, serde_json::Value>)]
    pub resource: Map<String, Value>,
    /// The buckets that have points, oldest first.
    pub buckets: Vec<Bucket>,
}

/// The points of one series in one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Bucket {
    /// The start of the bucket.
    #[schema(value_type = String, format = DateTime)]
    pub time: Timestamp,
    pub count: u64,
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    /// The value of the newest point.
    pub last: f64,
    /// The values a histogram recorded in the step: the counts of its
    /// buckets and percentile estimates. `None` for a gauge or a sum, and for
    /// the first step of a cumulative histogram, which only sets where the
    /// counting starts.
    #[schema(required = true)]
    pub histogram: Option<Distribution>,
}

impl Reader {
    /// The series with points in the range that `query` keeps, `limit` at
    /// most.
    ///
    /// # Errors
    ///
    /// When the query is invalid for metrics, or when it fails, such as when
    /// it runs past the time limit.
    pub fn metrics(&self, query: &Query, limit: usize) -> anyhow::Result<MetricList> {
        ensure!(
            query.signal == Signal::Metrics,
            "the query is over {}, not metrics",
            query.signal
        );
        let mut where_ = Filter::new();
        where_.push_clause(
            "EXISTS (SELECT 1 FROM $day.points p
                     WHERE p.series_id = s.id AND p.ts >= :since AND p.ts < :until)"
                .into(),
        );
        where_.param(":since", self.since());
        where_.param(":until", self.until());
        compile(query, ALIASES, self.indexes(), "q", &mut where_)?;
        let tail = format!(") ORDER BY name, service, labels LIMIT {}", limit + 1);
        let mut series = self.collect(
            ["SELECT DISTINCT * FROM (", &tail],
            |day| {
                format!(
                    "SELECT s.name, s.kind, s.unit, r.service, s.labels,
                            r.attributes AS resource
                     FROM {day}.series s JOIN {day}.resources r ON r.id = s.resource_id
                     WHERE {}",
                    where_.sql(day)
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
        let truncated = cut(&mut series, limit);
        Ok(MetricList { series, truncated })
    }

    /// The points of the series that pass `filter`, in buckets of its step,
    /// for `limit` series at most.
    ///
    /// # Errors
    ///
    /// When the step makes more than [`MAX_BUCKETS`] buckets, or when the
    /// query fails.
    pub fn metric(&self, filter: &MetricFilter, limit: usize) -> anyhow::Result<MetricSeries> {
        let step = filter.step_ns;
        ensure!(step > 0, "the step has to be longer than zero");
        ensure!(
            (self.until() - self.since()) / step <= MAX_BUCKETS,
            "the step makes more than {MAX_BUCKETS} buckets in the range; raise the step"
        );
        ensure!(
            filter.query.signal == Signal::Metrics,
            "the query is over {}, not metrics",
            filter.query.signal
        );
        let mut where_ = Filter::range(self, "p.ts");
        where_.push("s.name = :name", ":name", filter.name.clone());
        compile(&filter.query, ALIASES, self.indexes(), "q", &mut where_)?;
        let mut series: BTreeMap<Key, (BTreeMap<i64, Bucket>, Merger)> = BTreeMap::new();
        let mut truncated = false;
        self.scan(
            ["", " ORDER BY ts"],
            |day| {
                format!(
                    "SELECT r.service, s.kind, s.unit, s.labels, r.attributes, p.ts, p.value,
                            p.histogram
                     FROM {day}.points p
                     JOIN {day}.series s ON s.id = p.series_id
                     JOIN {day}.resources r ON r.id = s.resource_id
                     WHERE {}",
                    where_.sql(day)
                )
            },
            &where_,
            |row| {
                let key: Key = (
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                );
                let ts: i64 = row.get(5)?;
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
                let start = ts.div_euclid(step) * step;
                if let Some(histogram) = histogram {
                    merger.push(start, serde_json::from_str::<Histogram>(&histogram)?);
                }
                buckets
                    .entry(start)
                    .or_insert_with(|| Bucket::new(start, value))
                    .add(value);
                Ok(true)
            },
        )?;
        let series = series
            .into_iter()
            .map(|(key, (buckets, merger))| finish(key, buckets, merger))
            .collect::<anyhow::Result<_>>()?;
        Ok(MetricSeries {
            name: filter.name.clone(),
            step_ns: step,
            series,
            truncated,
        })
    }
}

impl Bucket {
    fn new(start: i64, value: f64) -> Self {
        Self {
            time: time(start),
            count: 0,
            min: value,
            max: value,
            avg: 0.0,
            last: value,
            histogram: None,
        }
    }

    fn add(&mut self, value: f64) {
        self.count += 1;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        // The sum until the end, when it becomes the average.
        self.avg += value;
        self.last = value;
    }
}

/// A series with the averages of its buckets and the distributions of its
/// histogram steps.
fn finish(
    (service, kind, unit, labels, resource): Key,
    mut buckets: BTreeMap<i64, Bucket>,
    merger: Merger,
) -> anyhow::Result<Series> {
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
        buckets: buckets
            .into_values()
            .map(|mut bucket| {
                #[expect(clippy::cast_precision_loss, reason = "a count below 2^53")]
                let count = bucket.count as f64;
                bucket.avg /= count;
                bucket
            })
            .collect(),
    })
}
