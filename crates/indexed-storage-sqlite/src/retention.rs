use std::collections::HashSet;

use otelo_query::Signal;
use rusqlite::params;

use crate::attribute_encoding::RecordGroupId;
use crate::catalog::AttributeOwner;
use crate::day::Day;
use crate::progress::Progress;
use crate::rollup::SummaryTable;
use crate::series::{MetricSeriesId, ResourceId};
use crate::telemetry_file::TelemetryFile;

// Small transactions keep the write-ahead log small, and a batch waits for one at most.
const MAX_DELETED_ROWS_PER_TRANSACTION: usize = 10_000;

const PAGES_PER_INCREMENTAL_VACUUM: i64 = 2_048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OldestRetainedDays {
    pub logs: Day,
    pub traces: Day,
    pub metrics: Day,
}

impl OldestRetainedDays {
    #[must_use]
    pub const fn of_signal(self, signal: Signal) -> Day {
        match signal {
            Signal::Logs => self.logs,
            Signal::Spans => self.traces,
            Signal::Metrics => self.metrics,
        }
    }

    // A resource belongs to the records of every signal, so it is counted as long as any.
    pub(crate) fn of_attribute_owner(self, owner: AttributeOwner) -> Day {
        match owner {
            AttributeOwner::Log => self.logs,
            AttributeOwner::Span => self.traces,
            AttributeOwner::MetricSeries => self.metrics,
            AttributeOwner::Resource => self.logs.min(self.traces).min(self.metrics),
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum RetentionStage {
    #[default]
    DeletingRows,
    VacuumingFreePages,
}

impl TelemetryFile {
    pub fn delete_next_past_retention(
        &mut self,
        oldest_retained_days: OldestRetainedDays,
    ) -> anyhow::Result<Progress> {
        if self.retention_stage == RetentionStage::VacuumingFreePages {
            return self.vacuum_next_free_pages();
        }
        let deleted_logs =
            self.delete_records_before("logs", "logged_at", oldest_retained_days.logs.start_at())?;
        if deleted_logs == MAX_DELETED_ROWS_PER_TRANSACTION {
            return Ok(Progress::MoreIsDue);
        }
        let deleted_spans = self.delete_records_before(
            "spans",
            "started_at",
            oldest_retained_days.traces.start_at(),
        )?;
        if deleted_spans == MAX_DELETED_ROWS_PER_TRANSACTION {
            return Ok(Progress::MoreIsDue);
        }
        if self.delete_metric_rows_before(oldest_retained_days.metrics.start_at())?
            == Progress::MoreIsDue
        {
            return Ok(Progress::MoreIsDue);
        }
        self.delete_unreferenced_rows_and_old_counts(oldest_retained_days)?;
        self.retention_stage = RetentionStage::VacuumingFreePages;
        self.vacuum_next_free_pages()
    }

    fn delete_records_before(
        &self,
        table: &str,
        instant_column: &str,
        oldest_retained_at: i64,
    ) -> anyhow::Result<usize> {
        let deleted_rows = self.connection().execute(
            &format!(
                "DELETE FROM {table}
                 WHERE rowid IN (SELECT rowid
                                 FROM {table}
                                 WHERE {instant_column} < ?1
                                 ORDER BY {instant_column}
                                 LIMIT {MAX_DELETED_ROWS_PER_TRANSACTION})"
            ),
            [oldest_retained_at],
        )?;
        if deleted_rows > 0 {
            tracing::debug!(table, deleted_rows, "deleted rows past the retention");
        }
        Ok(deleted_rows)
    }

    // One series at a time, so each DELETE reads the primary key.
    fn delete_metric_rows_before(&mut self, oldest_retained_at: i64) -> anyhow::Result<Progress> {
        let series_ids: Vec<MetricSeriesId> = self
            .connection()
            .prepare("SELECT id FROM metric_series ORDER BY id")?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let transaction = self.connection_mut().transaction()?;
        let mut deleted_rows = 0;
        for series_id in series_ids {
            deleted_rows += transaction
                .prepare_cached(
                    "DELETE FROM metric_points
                     WHERE metric_series_id = ?1 AND recorded_at < ?2",
                )?
                .execute(params![series_id, oldest_retained_at])?;
            for table in [SummaryTable::Minute, SummaryTable::Hour] {
                deleted_rows += transaction
                    .prepare_cached(&format!(
                        "DELETE FROM {}
                         WHERE metric_series_id = ?1 AND start_at < ?2",
                        table.name()
                    ))?
                    .execute(params![series_id, oldest_retained_at])?;
            }
            if deleted_rows >= MAX_DELETED_ROWS_PER_TRANSACTION {
                transaction.commit()?;
                return Ok(Progress::MoreIsDue);
            }
        }
        transaction.commit()?;
        Ok(Progress::CaughtUp)
    }

    fn delete_unreferenced_rows_and_old_counts(
        &mut self,
        oldest_retained_days: OldestRetainedDays,
    ) -> anyhow::Result<()> {
        let transaction = self.connection_mut().transaction()?;
        let deleted_series_ids: HashSet<MetricSeriesId> = transaction
            .prepare(
                "DELETE FROM metric_series
                 WHERE NOT EXISTS (SELECT 1
                                   FROM metric_points metric_point
                                   WHERE metric_point.metric_series_id = metric_series.id)
                   AND NOT EXISTS (SELECT 1
                                   FROM metric_minute_summaries summary
                                   WHERE summary.metric_series_id = metric_series.id)
                   AND NOT EXISTS (SELECT 1
                                   FROM metric_hour_summaries summary
                                   WHERE summary.metric_series_id = metric_series.id)
                 RETURNING id",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        // A set or a value holds the day of its newest record, so no record past the retention
        // keeps one.
        for owner in [AttributeOwner::Log, AttributeOwner::Span] {
            transaction
                .prepare_cached(
                    "DELETE FROM stable_attribute_sets
                     WHERE newest_record_day < ?2
                       AND record_group_id IN (SELECT id
                                               FROM record_groups
                                               WHERE attribute_owner = ?1)",
                )?
                .execute(params![
                    owner.name(),
                    oldest_retained_days.of_attribute_owner(owner)
                ])?;
        }
        transaction.execute(
            "DELETE FROM interned_attribute_values WHERE newest_record_day < ?1",
            [oldest_retained_days.logs.min(oldest_retained_days.traces)],
        )?;
        transaction.execute(
            "DELETE FROM attribute_key_profiles AS attribute_key_profile
             WHERE NOT EXISTS (SELECT 1
                               FROM stable_attribute_sets stable_attribute_set
                               WHERE stable_attribute_set.record_group_id
                                       = attribute_key_profile.record_group_id)",
            [],
        )?;
        let deleted_group_ids: HashSet<RecordGroupId> = transaction
            .prepare(
                "DELETE FROM record_groups AS record_group
                 WHERE NOT EXISTS (SELECT 1
                                   FROM stable_attribute_sets stable_attribute_set
                                   WHERE stable_attribute_set.record_group_id = record_group.id)
                 RETURNING id",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let deleted_resource_ids: HashSet<ResourceId> = transaction
            .prepare(
                "DELETE FROM resources AS resource
                 WHERE NOT EXISTS (SELECT 1
                                   FROM metric_series
                                   WHERE metric_series.resource_id = resource.id)
                   AND NOT EXISTS (SELECT 1
                                   FROM record_groups record_group
                                   WHERE record_group.resource_id = resource.id)
                 RETURNING id",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        for owner in [
            AttributeOwner::Log,
            AttributeOwner::Span,
            AttributeOwner::MetricSeries,
            AttributeOwner::Resource,
        ] {
            let oldest_retained_day = oldest_retained_days.of_attribute_owner(owner);
            for table in ["attribute_key_counts", "attribute_value_counts"] {
                transaction
                    .prepare_cached(&format!(
                        "DELETE FROM {table} WHERE attribute_owner = ?1 AND day < ?2"
                    ))?
                    .execute(params![owner.name(), oldest_retained_day])?;
            }
        }
        transaction.execute(
            "DELETE FROM span_name_counts WHERE day < ?1",
            [oldest_retained_days.traces],
        )?;
        transaction.commit()?;
        if !deleted_series_ids.is_empty() || !deleted_resource_ids.is_empty() {
            tracing::debug!(
                deleted_series = deleted_series_ids.len(),
                deleted_resources = deleted_resource_ids.len(),
                "deleted the series and resources past the retention"
            );
        }
        self.forget_rows_past_retention(
            oldest_retained_days.of_attribute_owner(AttributeOwner::Resource),
            &deleted_resource_ids,
            &deleted_series_ids,
            &deleted_group_ids,
        );
        Ok(())
    }

    // New rows reuse the free pages, so the file only gives back what a lowered retention freed.
    fn vacuum_next_free_pages(&mut self) -> anyhow::Result<Progress> {
        let free_page_count_before = self.read_free_page_count()?;
        let page_count: i64 = self
            .connection()
            .pragma_query_value(None, "page_count", |row| row.get(0))?;
        if free_page_count_before * 4 > page_count {
            // The pragma frees a page each time it steps, so it steps to its end.
            let mut statement = self.connection().prepare(&format!(
                "PRAGMA incremental_vacuum({PAGES_PER_INCREMENTAL_VACUUM})"
            ))?;
            let mut rows = statement.query([])?;
            while rows.next()?.is_some() {}
            drop(rows);
            drop(statement);
            // A file made without auto_vacuum frees nothing.
            if self.read_free_page_count()? < free_page_count_before {
                return Ok(Progress::MoreIsDue);
            }
        }
        self.retention_stage = RetentionStage::DeletingRows;
        Ok(Progress::CaughtUp)
    }

    fn read_free_page_count(&self) -> anyhow::Result<i64> {
        Ok(self
            .connection()
            .pragma_query_value(None, "freelist_count", |row| row.get(0))?)
    }
}
