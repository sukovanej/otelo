use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use otelo_indexed_storage::query::Resolution;
use otelo_indexed_storage::{
    Change, Increase, Level, MetricKind, SeriesSteps, StepSummary, Temporality, TimeRange,
};
use rusqlite::{OptionalExtension, Row, Transaction, params};

use crate::progress::Progress;
use crate::query::{BASELINE_LOOKBACK_NS, metric_kind_from_stored_names, read_series_point};
use crate::series::MetricSeriesId;
use crate::telemetry_file::TelemetryFile;

pub const MINUTE_NS: i64 = 60 * 1_000_000_000;
pub const HOUR_NS: i64 = 60 * MINUTE_NS;

// A batch may reach the writer a while after its points were recorded.
const LATE_BATCH_WAIT_NS: i64 = 2 * MINUTE_NS;

const SUMMARY_COLUMNS: &str = "point_count, min_value, max_value, value_sum, last_value, \
                               counter_increase, counter_increase_seconds, merged_histogram";

#[derive(Clone, Copy)]
pub enum SummaryTable {
    Minute,
    Hour,
}

impl SummaryTable {
    pub const fn from_resolution(resolution: Resolution) -> Option<Self> {
        match resolution {
            Resolution::Raw => None,
            Resolution::Minute => Some(Self::Minute),
            Resolution::Hour => Some(Self::Hour),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Minute => "metric_minute_summaries",
            Self::Hour => "metric_hour_summaries",
        }
    }

    pub const fn step_ns(self) -> i64 {
        match self {
            Self::Minute => MINUTE_NS,
            Self::Hour => HOUR_NS,
        }
    }
}

impl TelemetryFile {
    // At most an hour of minutes and one hour at a time, so the writer takes batches in between.
    pub fn roll_up_next_due(&mut self, now: i64, oldest_point_at: i64) -> anyhow::Result<Progress> {
        let minutes_due_until = (now - LATE_BATCH_WAIT_NS).div_euclid(MINUTE_NS) * MINUTE_NS;
        let Some(minutes_summarized_until) = self.read_minutes_summarized_until(oldest_point_at)?
        else {
            return Ok(Progress::CaughtUp);
        };
        let minutes_summarized_until = if minutes_summarized_until < minutes_due_until {
            let end_of_hour = (minutes_summarized_until.div_euclid(HOUR_NS) + 1) * HOUR_NS;
            let minute_range =
                TimeRange::new(minutes_summarized_until, end_of_hour.min(minutes_due_until))?;
            self.summarize_minutes(minute_range)?;
            minute_range.end_at()
        } else {
            minutes_summarized_until
        };

        let hours_due_until = minutes_summarized_until.div_euclid(HOUR_NS) * HOUR_NS;
        let hours_summarized_until = match self.read_summarized_until(SummaryTable::Hour)? {
            Some(summarized_until) => summarized_until,
            None => self
                .read_start_of_first_minute()?
                .map_or(hours_due_until, |first_minute_start_at| {
                    first_minute_start_at.div_euclid(HOUR_NS) * HOUR_NS
                }),
        };
        let hours_summarized_until = if hours_summarized_until < hours_due_until {
            self.summarize_hour(hours_summarized_until)?;
            hours_summarized_until + HOUR_NS
        } else {
            hours_summarized_until
        };
        Ok(
            if minutes_summarized_until < minutes_due_until
                || hours_summarized_until < hours_due_until
            {
                Progress::MoreIsDue
            } else {
                Progress::CaughtUp
            },
        )
    }

    fn read_minutes_summarized_until(&self, oldest_point_at: i64) -> anyhow::Result<Option<i64>> {
        let summarized_until = match self.read_summarized_until(SummaryTable::Minute)? {
            Some(summarized_until) => Some(summarized_until),
            None => self
                .connection()
                .query_row("SELECT min(recorded_at) FROM metric_points", [], |row| {
                    row.get::<_, Option<i64>>(0)
                })?
                .map(|first_recorded_at| first_recorded_at.div_euclid(MINUTE_NS) * MINUTE_NS),
        };
        // A daemon that was down for longer than the points are kept starts again at the oldest
        // of them.
        Ok(summarized_until.map(|summarized_until| summarized_until.max(oldest_point_at)))
    }

    fn read_summarized_until(&self, table: SummaryTable) -> anyhow::Result<Option<i64>> {
        Ok(self
            .connection()
            .prepare_cached(
                "SELECT summarized_until FROM metric_summary_progress WHERE summary_table = ?1",
            )?
            .query_row([table.name()], |row| row.get(0))
            .optional()?)
    }

    fn read_start_of_first_minute(&self) -> anyhow::Result<Option<i64>> {
        Ok(self.connection().query_row(
            "SELECT min(start_at) FROM metric_minute_summaries",
            [],
            |row| row.get(0),
        )?)
    }

    fn summarize_minutes(&mut self, minute_range: TimeRange) -> anyhow::Result<()> {
        let steps_by_series = self.sum_up_points_by_minute(minute_range)?;
        let transaction = self.connection_mut().transaction()?;
        for (series_id, series_steps) in steps_by_series {
            if series_steps.has_no_point_in_range() {
                continue;
            }
            for (minute_start_at, summary) in series_steps.into_summaries_by_step() {
                write_summary(
                    &transaction,
                    SummaryTable::Minute,
                    series_id,
                    minute_start_at,
                    &summary,
                )?;
            }
        }
        write_summarized_until(&transaction, SummaryTable::Minute, minute_range.end_at())?;
        transaction.commit()?;
        Ok(())
    }

    fn sum_up_points_by_minute(
        &self,
        minute_range: TimeRange,
    ) -> anyhow::Result<BTreeMap<MetricSeriesId, SeriesSteps>> {
        let since = minute_range.start_at().saturating_sub(BASELINE_LOOKBACK_NS);
        // CROSS JOIN keeps metric_series the outer loop, so the points come by their primary key.
        let mut statement = self.connection().prepare_cached(
            "SELECT metric_series.id, metric_series.kind, metric_series.aggregation_temporality,
                    metric_point.recorded_at, metric_point.value, metric_point.histogram
             FROM metric_series
             CROSS JOIN metric_points metric_point ON metric_point.metric_series_id = metric_series.id
             WHERE metric_point.recorded_at >= ?1 AND metric_point.recorded_at < ?2
             ORDER BY metric_series.id, metric_point.recorded_at",
        )?;
        let mut rows = statement.query([since, minute_range.end_at()])?;
        let mut steps_by_series: BTreeMap<MetricSeriesId, SeriesSteps> = BTreeMap::new();
        while let Some(row) = rows.next()? {
            let kind: String = row.get(1)?;
            let aggregation_temporality: Option<String> = row.get(2)?;
            let kind = metric_kind_from_stored_names(&kind, aggregation_temporality.as_deref())?;
            let point = read_series_point(row, 3)?;
            let series_steps = steps_by_series
                .entry(row.get(0)?)
                .or_insert_with(|| SeriesSteps::new(kind));
            let minute_start_at = point.recorded_at().div_euclid(MINUTE_NS) * MINUTE_NS;
            if point.recorded_at() >= minute_range.start_at() {
                series_steps.add_point(minute_start_at, point);
            } else {
                series_steps.add_point_before_range(minute_start_at, point);
            }
        }
        Ok(steps_by_series)
    }

    fn summarize_hour(&mut self, hour_start_at: i64) -> anyhow::Result<()> {
        let transaction = self.connection_mut().transaction()?;
        let mut summaries_by_series: BTreeMap<MetricSeriesId, StepSummary> = BTreeMap::new();
        {
            // CROSS JOIN keeps metric_series the outer loop, so the minutes come by their primary
            // key.
            let mut select_minutes = transaction.prepare_cached(&format!(
                "SELECT summary.metric_series_id, {}
                 FROM metric_series
                 CROSS JOIN metric_minute_summaries summary
                   ON summary.metric_series_id = metric_series.id
                 WHERE summary.start_at >= ?1 AND summary.start_at < ?2
                 ORDER BY summary.metric_series_id, summary.start_at",
                summary_columns_of("summary")
            ))?;
            let mut rows = select_minutes.query([hour_start_at, hour_start_at + HOUR_NS])?;
            while let Some(row) = rows.next()? {
                let minute_summary = read_summary(row, 1)?;
                match summaries_by_series.entry(row.get(0)?) {
                    Entry::Occupied(entry) => entry.into_mut().add_later_summary(minute_summary),
                    Entry::Vacant(entry) => {
                        entry.insert(minute_summary);
                    }
                }
            }
        }
        for (series_id, summary) in summaries_by_series {
            write_summary(
                &transaction,
                SummaryTable::Hour,
                series_id,
                hour_start_at,
                &summary,
            )?;
        }
        write_summarized_until(&transaction, SummaryTable::Hour, hour_start_at + HOUR_NS)?;
        transaction.commit()?;
        Ok(())
    }
}

// A summary holds what each step added, whatever the points of the series counted.
pub const fn convert_to_summary_kind(kind: MetricKind) -> MetricKind {
    match kind {
        MetricKind::Gauge | MetricKind::UpDown => kind,
        MetricKind::Counter(_) => MetricKind::Counter(Temporality::Delta),
        MetricKind::Histogram(_) => MetricKind::Histogram(Temporality::Delta),
    }
}

fn write_summarized_until(
    transaction: &Transaction,
    table: SummaryTable,
    summarized_until: i64,
) -> anyhow::Result<()> {
    transaction
        .prepare_cached(
            "INSERT OR REPLACE INTO metric_summary_progress (summary_table, summarized_until)
             VALUES (?1, ?2)",
        )?
        .execute(params![table.name(), summarized_until])?;
    Ok(())
}

// A step summarized again, after a stop before its progress was saved, replaces its row.
fn write_summary(
    transaction: &Transaction,
    table: SummaryTable,
    series_id: MetricSeriesId,
    start_at: i64,
    summary: &StepSummary,
) -> anyhow::Result<()> {
    let (counter_increase, counter_increase_seconds, merged_histogram_json) = match &summary.change
    {
        Change::Nothing => (None, None, None),
        Change::Increase(increase) => (
            Some(increase.counter_increase),
            Some(increase.counter_increase_seconds),
            None,
        ),
        Change::Distribution(histogram) => (None, None, Some(serde_json::to_string(histogram)?)),
    };
    transaction
        .prepare_cached(&format!(
            "INSERT OR REPLACE INTO {} (metric_series_id, start_at, {SUMMARY_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            table.name()
        ))?
        .execute(params![
            series_id,
            start_at,
            summary.level.point_count.cast_signed(),
            summary.level.min_value,
            summary.level.max_value,
            summary.level.value_sum,
            summary.level.last_value,
            counter_increase,
            counter_increase_seconds,
            merged_histogram_json,
        ])?;
    Ok(())
}

pub fn read_summary(row: &Row, first_summary_column: usize) -> anyhow::Result<StepSummary> {
    let point_count: i64 = row.get(first_summary_column)?;
    let counter_increase: Option<f64> = row.get(first_summary_column + 5)?;
    let counter_increase_seconds: Option<f64> = row.get(first_summary_column + 6)?;
    let merged_histogram_json: Option<String> = row.get(first_summary_column + 7)?;
    let change = match (
        counter_increase,
        counter_increase_seconds,
        merged_histogram_json,
    ) {
        (Some(counter_increase), Some(counter_increase_seconds), _) => Change::Increase(Increase {
            counter_increase,
            counter_increase_seconds,
        }),
        (_, _, Some(json)) => Change::Distribution(Box::new(serde_json::from_str(&json)?)),
        _ => Change::Nothing,
    };
    Ok(StepSummary {
        level: Level {
            point_count: point_count.cast_unsigned(),
            min_value: row.get(first_summary_column + 1)?,
            max_value: row.get(first_summary_column + 2)?,
            value_sum: row.get(first_summary_column + 3)?,
            last_value: row.get(first_summary_column + 4)?,
        },
        change,
    })
}

pub fn summary_columns_of(table_alias: &str) -> String {
    SUMMARY_COLUMNS
        .split(", ")
        .map(|column| format!("{table_alias}.{column}"))
        .collect::<Vec<_>>()
        .join(", ")
}
