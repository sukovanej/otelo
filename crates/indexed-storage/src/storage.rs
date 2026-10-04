use std::collections::BTreeSet;
use std::time::Duration;

use otelo_query::{Catalog, Query, Signal};

use crate::query::{
    AttributeKeys, GroupBuckets, LogCounts, LogGroupingField, LogGroups, Logs, MetricFilter,
    MetricList, MetricSeries, RankOrder, Resolution, Service, Services, SpanGroupRanking,
    SpanGroupingField, SpanGroups, SpanSort, Spans, Trace, Traces,
};
use crate::{IndexedAttribute, Result, TimeRange, TraceId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageSize {
    pub journal_bytes: u64,
    pub telemetry_bytes: u64,
    pub state_bytes: u64,
}

pub trait Storage: Send + Sync {
    fn oldest_retained_at(&self, signal: Signal) -> i64;

    fn size_in_bytes(&self) -> Result<u64>;

    fn open_range(&self, range: TimeRange, time_limit: Duration) -> Result<Box<dyn RangeQueries>>;

    fn indexed_attributes(&self) -> BTreeSet<IndexedAttribute>;

    fn replace_indexed_attributes(&self, attributes: BTreeSet<IndexedAttribute>);
}

pub trait RangeQueries: Catalog {
    fn list_logs(&self, query: &Query, limit: usize) -> Result<Logs>;

    fn list_log_groups(&self, query: &Query, limit: usize) -> Result<LogGroups>;

    fn count_logs(
        &self,
        query: &Query,
        by: &[LogGroupingField],
        order: RankOrder,
        step_ns: i64,
        limit: usize,
    ) -> Result<LogCounts>;

    fn list_spans(&self, query: &Query, sort: SpanSort, limit: usize) -> Result<Spans>;

    fn list_span_groups(
        &self,
        query: &Query,
        by: &[SpanGroupingField],
        ranking: SpanGroupRanking,
        step_ns: i64,
        group_buckets: GroupBuckets,
        limit: usize,
    ) -> Result<SpanGroups>;

    fn list_traces(&self, query: &Query, sort: SpanSort, limit: usize) -> Result<Traces>;

    fn get_trace(&self, trace_id: TraceId, limit: usize) -> Result<Option<Trace>>;

    fn list_metrics(
        &self,
        query: &Query,
        resolution: Resolution,
        limit: usize,
    ) -> Result<MetricList>;

    fn get_metric_series(&self, filter: &MetricFilter, limit: usize) -> Result<MetricSeries>;

    fn list_services(&self, step_ns: i64, limit: usize) -> Result<Services>;

    fn get_service(&self, service: &str, step_ns: i64) -> Result<Service>;

    fn list_attribute_keys(&self, signal: Signal) -> Result<AttributeKeys>;

    fn explain_query(&self, query: &Query) -> Result<Vec<String>>;
}
