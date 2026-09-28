use std::collections::{BTreeMap, btree_map::Entry};

use anyhow::ensure;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::ToSchema;

use super::{Filter, cut, time};
use crate::Reader;

/// The service, kind, unit, and labels of a series. Series from different
/// day files are one series when these match.
type Key = (String, String, String, String);

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
    let range = until.saturating_sub(since);
    STEPS
        .into_iter()
        .find(|step| range / step <= 120)
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
    #[schema(value_type = Object)]
    pub labels: Map<String, Value>,
}

#[derive(Clone, Debug)]
pub struct MetricFilter {
    pub name: String,
    pub service: Option<String>,
    /// Label names and the values they must have.
    pub labels: Vec<(String, String)>,
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
    #[schema(value_type = Object)]
    pub labels: Map<String, Value>,
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
}

impl Reader {
    /// The series that have points in the range, `limit` at most.
    ///
    /// # Errors
    ///
    /// When the query fails, such as when it runs past the time limit.
    pub fn metrics(&self, service: Option<&str>, limit: usize) -> anyhow::Result<MetricList> {
        let mut where_ = Filter::new();
        where_.push_clause(
            "EXISTS (SELECT 1 FROM $day.points p
                     WHERE p.series_id = s.id AND p.ts >= :since AND p.ts < :until)"
                .into(),
        );
        where_.param(":since", self.since());
        where_.param(":until", self.until());
        if let Some(service) = service {
            where_.push("r.service = :service", ":service", service.to_owned());
        }
        let tail = format!(") ORDER BY name, service, labels LIMIT {}", limit + 1);
        let mut series = self.collect(
            ["SELECT DISTINCT * FROM (", &tail],
            |day| {
                format!(
                    "SELECT s.name, s.kind, s.unit, r.service, s.labels
                     FROM {day}.series s JOIN {day}.resources r ON r.id = s.resource_id
                     WHERE {}",
                    where_.sql(day)
                )
            },
            &where_,
            |row| {
                let labels: String = row.get(4)?;
                Ok(SeriesInfo {
                    name: row.get(0)?,
                    kind: row.get(1)?,
                    unit: row.get(2)?,
                    service: row.get(3)?,
                    labels: serde_json::from_str(&labels)?,
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
        let mut where_ = Filter::range(self, "p.ts");
        where_.push("s.name = :name", ":name", filter.name.clone());
        if let Some(service) = &filter.service {
            where_.push("r.service = :service", ":service", service.clone());
        }
        for (i, (label, value)) in filter.labels.iter().enumerate() {
            let path = format!("$.\"{}\"", label.replace('"', "\\\""));
            where_.push(
                &format!("CAST(json_extract(s.labels, :label{i}) AS TEXT) = :value{i}"),
                &format!(":label{i}"),
                path,
            );
            where_.param(&format!(":value{i}"), value.clone());
        }
        let mut series: BTreeMap<Key, BTreeMap<i64, Bucket>> = BTreeMap::new();
        let mut truncated = false;
        self.scan(
            ["", " ORDER BY ts"],
            |day| {
                format!(
                    "SELECT r.service, s.kind, s.unit, s.labels, p.ts, p.value
                     FROM {day}.points p
                     JOIN {day}.series s ON s.id = p.series_id
                     JOIN {day}.resources r ON r.id = s.resource_id
                     WHERE {}",
                    where_.sql(day)
                )
            },
            &where_,
            |row| {
                let key: Key = (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?);
                let ts: i64 = row.get(4)?;
                let value: f64 = row.get(5)?;
                let len = series.len();
                let buckets = match series.entry(key) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(_) if len == limit => {
                        truncated = true;
                        return Ok(true);
                    }
                    Entry::Vacant(entry) => entry.insert(BTreeMap::new()),
                };
                let start = ts.div_euclid(step) * step;
                let bucket = buckets.entry(start).or_insert_with(|| Bucket {
                    time: time(start),
                    count: 0,
                    min: value,
                    max: value,
                    avg: 0.0,
                    last: value,
                });
                bucket.count += 1;
                bucket.min = bucket.min.min(value);
                bucket.max = bucket.max.max(value);
                // The sum until the end, when it becomes the average.
                bucket.avg += value;
                bucket.last = value;
                Ok(true)
            },
        )?;
        let series = series
            .into_iter()
            .map(|((service, kind, unit, labels), buckets)| {
                Ok(Series {
                    service,
                    kind,
                    unit,
                    labels: serde_json::from_str(&labels)?,
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
            })
            .collect::<anyhow::Result<_>>()?;
        Ok(MetricSeries {
            name: filter.name.clone(),
            step_ns: step,
            series,
            truncated,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_a_step_that_fits_the_range() {
        assert_eq!(default_step(0, 3600 * SECOND), 30 * SECOND);
        assert_eq!(default_step(0, 60 * SECOND), SECOND);
        assert_eq!(default_step(0, 7 * 86_400 * SECOND), 3 * 3600 * SECOND);
        assert_eq!(default_step(0, 1000 * 86_400 * SECOND), 86_400 * SECOND);
    }
}
