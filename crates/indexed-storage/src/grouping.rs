use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use jiff::Timestamp;

use crate::query::{Bucket, BucketChange, GroupKey, Grouping, GroupingField, SeriesGroup};
use crate::{AttributeValue, Attributes, Change, Distribution, Histogram, MetricKind, StepSummary};

pub struct SummarizedSeries {
    pub service: String,
    pub kind: MetricKind,
    pub unit: String,
    pub attributes: Attributes,
    pub resource: Attributes,
    pub summaries_by_step: BTreeMap<i64, StepSummary>,
}

impl SummarizedSeries {
    fn values_of(&self, by: &[GroupingField]) -> Attributes {
        let mut values = Attributes::new();
        for field in by {
            let value = match field {
                GroupingField::Service => Some(AttributeValue::from(self.service.as_str())),
                GroupingField::Attribute(key) => self.attributes.get(key).cloned(),
                GroupingField::Resource(key) => self.resource.get(key).cloned(),
            };
            if let Some(value) = value {
                values.insert(field.to_string(), value);
            }
        }
        values
    }
}

#[must_use]
pub fn group_series(series: Vec<SummarizedSeries>, grouping: &Grouping) -> Vec<SeriesGroup> {
    let groups = if grouping.by.is_empty() {
        series.into_iter().map(keep_series_alone).collect()
    } else {
        group_series_by_values(series, &grouping.by)
    };
    let mut ranked_groups: Vec<(f64, GroupIdentity, CombinedSeries)> = groups
        .into_iter()
        .map(|(identity, group)| (group.value_over_range(), identity, group))
        .collect();
    ranked_groups.sort_by(|(value, ..), (other_value, ..)| other_value.total_cmp(value));
    let groups = ranked_groups
        .into_iter()
        .map(|(_, identity, group)| (identity, group));
    let Some(top) = grouping.top else {
        return groups
            .map(|(identity, group)| {
                let key = identity.into_group_key(group.series_count);
                group.into_series_group(key)
            })
            .collect();
    };
    let mut kept_counts: BTreeMap<(MetricKind, String), usize> = BTreeMap::new();
    let mut others: BTreeMap<(MetricKind, String), (usize, CombinedSeries)> = BTreeMap::new();
    let mut kept_groups = Vec::new();
    for (identity, group) in groups {
        let kind_and_unit = (group.kind, group.unit.clone());
        let kept_count = kept_counts.entry(kind_and_unit.clone()).or_default();
        if *kept_count < top.get() {
            *kept_count += 1;
            let key = identity.into_group_key(group.series_count);
            kept_groups.push(group.into_series_group(key));
            continue;
        }
        match others.entry(kind_and_unit) {
            Entry::Occupied(entry) => {
                let (group_count, other) = entry.into_mut();
                *group_count += 1;
                other.add_combined_series(group);
            }
            Entry::Vacant(entry) => {
                entry.insert((1, group));
            }
        }
    }
    kept_groups.extend(others.into_values().map(|(group_count, other)| {
        let key = GroupKey::Other {
            group_count,
            series_count: other.series_count,
        };
        other.into_series_group(key)
    }));
    kept_groups
}

fn keep_series_alone(series: SummarizedSeries) -> (GroupIdentity, CombinedSeries) {
    let mut combined = CombinedSeries::new(series.kind, series.unit);
    combined.add_summaries_of_series(series.summaries_by_step);
    let identity = GroupIdentity::Series {
        service: series.service,
        attributes: series.attributes,
        resource: series.resource,
    };
    (identity, combined)
}

fn group_series_by_values(
    series: Vec<SummarizedSeries>,
    by: &[GroupingField],
) -> Vec<(GroupIdentity, CombinedSeries)> {
    let mut groups: BTreeMap<(MetricKind, String, String), (GroupIdentity, CombinedSeries)> =
        BTreeMap::new();
    for one_series in series {
        let values = one_series.values_of(by);
        let group_key = (one_series.kind, one_series.unit.clone(), values.to_json());
        let (_, combined) = groups.entry(group_key).or_insert_with(|| {
            (
                GroupIdentity::Values(values),
                CombinedSeries::new(one_series.kind, one_series.unit),
            )
        });
        combined.add_summaries_of_series(one_series.summaries_by_step);
    }
    groups.into_values().collect()
}

enum GroupIdentity {
    Series {
        service: String,
        attributes: Attributes,
        resource: Attributes,
    },
    Values(Attributes),
}

impl GroupIdentity {
    fn into_group_key(self, series_count: usize) -> GroupKey {
        match self {
            Self::Series {
                service,
                attributes,
                resource,
            } => GroupKey::Series {
                service,
                attributes,
                resource,
            },
            Self::Values(values) => GroupKey::Values {
                values,
                series_count,
            },
        }
    }
}

#[derive(Clone, Copy)]
enum LevelCombination {
    Average,
    Sum,
}

impl LevelCombination {
    const fn of(kind: MetricKind) -> Self {
        match kind {
            MetricKind::Gauge => Self::Average,
            MetricKind::UpDown | MetricKind::Counter(_) | MetricKind::Histogram(_) => Self::Sum,
        }
    }
}

struct CombinedSeries {
    kind: MetricKind,
    unit: String,
    series_count: usize,
    steps: BTreeMap<i64, CombinedStep>,
}

impl CombinedSeries {
    const fn new(kind: MetricKind, unit: String) -> Self {
        Self {
            kind,
            unit,
            series_count: 0,
            steps: BTreeMap::new(),
        }
    }

    fn add_summaries_of_series(&mut self, summaries_by_step: BTreeMap<i64, StepSummary>) {
        self.series_count += 1;
        for (step_start_at, summary) in summaries_by_step {
            self.add_step(step_start_at, CombinedStep::from_step_summary(summary));
        }
    }

    fn add_combined_series(&mut self, other: Self) {
        self.series_count += other.series_count;
        for (step_start_at, step) in other.steps {
            self.add_step(step_start_at, step);
        }
    }

    fn add_step(&mut self, step_start_at: i64, step: CombinedStep) {
        let combination = LevelCombination::of(self.kind);
        match self.steps.entry(step_start_at) {
            Entry::Occupied(entry) => entry.into_mut().add_step(step, combination),
            Entry::Vacant(entry) => {
                entry.insert(step);
            }
        }
    }

    fn value_over_range(&self) -> f64 {
        let combination = LevelCombination::of(self.kind);
        match self.kind {
            MetricKind::Gauge | MetricKind::UpDown => {
                average_of(self.steps.values().map(|step| step.average(combination)))
            }
            MetricKind::Counter(_) => {
                average_of(self.steps.values().filter_map(|step| match step.change {
                    CombinedChange::Rate { per_second } => Some(per_second),
                    CombinedChange::Nothing | CombinedChange::Distribution(_) => None,
                }))
            }
            MetricKind::Histogram(_) => self
                .steps
                .values()
                .filter_map(|step| match &step.change {
                    CombinedChange::Distribution(histogram) => histogram.sum,
                    CombinedChange::Nothing | CombinedChange::Rate { .. } => None,
                })
                .sum(),
        }
    }

    fn into_series_group(self, key: GroupKey) -> SeriesGroup {
        let combination = LevelCombination::of(self.kind);
        SeriesGroup {
            key,
            kind: self.kind,
            unit: self.unit,
            buckets: self
                .steps
                .into_iter()
                .map(|(step_start_at, step)| step.into_bucket(step_start_at, combination))
                .collect(),
        }
    }
}

fn average_of(values: impl Iterator<Item = f64>) -> f64 {
    let (sum, count) = values.fold((0.0, 0_u32), |(sum, count), value| (sum + value, count + 1));
    if count == 0 {
        0.0
    } else {
        sum / f64::from(count)
    }
}

struct CombinedStep {
    series_count: u64,
    point_count: u64,
    min: f64,
    max: f64,
    sum_of_averages: f64,
    sum_of_lasts: f64,
    change: CombinedChange,
}

enum CombinedChange {
    Nothing,
    Rate { per_second: f64 },
    Distribution(Box<Histogram>),
}

impl CombinedStep {
    fn from_step_summary(summary: StepSummary) -> Self {
        let change = match summary.change {
            Change::Nothing => CombinedChange::Nothing,
            Change::Increase(increase) => increase
                .rate_per_second()
                .map_or(CombinedChange::Nothing, |per_second| CombinedChange::Rate {
                    per_second,
                }),
            Change::Distribution(histogram) => CombinedChange::Distribution(histogram),
        };
        Self {
            series_count: 1,
            point_count: summary.level.count,
            min: summary.level.min,
            max: summary.level.max,
            sum_of_averages: summary.level.average(),
            sum_of_lasts: summary.level.last,
            change,
        }
    }

    fn add_step(&mut self, other: Self, combination: LevelCombination) {
        self.series_count += other.series_count;
        self.point_count += other.point_count;
        match combination {
            LevelCombination::Average => {
                self.min = self.min.min(other.min);
                self.max = self.max.max(other.max);
            }
            LevelCombination::Sum => {
                self.min += other.min;
                self.max += other.max;
            }
        }
        self.sum_of_averages += other.sum_of_averages;
        self.sum_of_lasts += other.sum_of_lasts;
        match (&mut self.change, other.change) {
            (_, CombinedChange::Nothing) => {}
            (
                CombinedChange::Rate { per_second },
                CombinedChange::Rate {
                    per_second: other_per_second,
                },
            ) => *per_second += other_per_second,
            (CombinedChange::Distribution(histogram), CombinedChange::Distribution(other)) => {
                histogram.add_increase(*other);
            }
            (change, other_change) => *change = other_change,
        }
    }

    #[expect(clippy::cast_precision_loss, reason = "a count of series below 2^53")]
    fn combine_sum_over_series(&self, sum: f64, combination: LevelCombination) -> f64 {
        match combination {
            LevelCombination::Average => sum / self.series_count as f64,
            LevelCombination::Sum => sum,
        }
    }

    fn average(&self, combination: LevelCombination) -> f64 {
        self.combine_sum_over_series(self.sum_of_averages, combination)
    }

    fn into_bucket(self, step_start_at: i64, combination: LevelCombination) -> Bucket {
        Bucket {
            start_at: Timestamp::from_nanosecond(i128::from(step_start_at))
                .expect("an i64 of nanoseconds is a valid timestamp"),
            count: self.point_count,
            min: self.min,
            max: self.max,
            avg: self.average(combination),
            last: self.combine_sum_over_series(self.sum_of_lasts, combination),
            change: match self.change {
                CombinedChange::Nothing => BucketChange::None,
                CombinedChange::Rate { per_second } => BucketChange::Rate { per_second },
                CombinedChange::Distribution(histogram) => {
                    BucketChange::Distribution(Distribution::from(*histogram))
                }
            },
        }
    }
}
