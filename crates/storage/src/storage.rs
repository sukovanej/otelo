use std::collections::BTreeSet;
use std::time::Duration;

use otelo_query::{Catalog, Query, Signal};

use crate::query::{
    AttributeKeys, CallDetail, Calls, LogGroups, Logs, MetricFilter, MetricList, MetricSeries,
    OperationDetail, Resolution, Service, Services, Spans, SqlResult, TargetKey, Trace, Traces,
};
use crate::{IndexedAttribute, Result, SpanKind, TimeRange, TraceId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageSize {
    pub telemetry_bytes: u64,
    pub rollup_bytes: u64,
    pub state_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetricRetention {
    pub oldest_raw_at: i64,
    pub oldest_minute_at: i64,
    pub oldest_hour_at: i64,
}

pub trait Storage: Send + Sync {
    fn oldest_retained_at(&self) -> i64;

    fn metric_retention(&self) -> MetricRetention;

    fn size(&self) -> Result<StorageSize>;

    fn open_range(&self, range: TimeRange, time_limit: Duration) -> Result<Box<dyn RangeQueries>>;

    fn indexed_attributes(&self) -> BTreeSet<IndexedAttribute>;

    fn add_index(&self, attribute: &IndexedAttribute) -> Result<()>;

    fn remove_index(&self, attribute: &IndexedAttribute) -> Result<bool>;
}

pub trait RangeQueries: Catalog {
    fn logs(&self, query: &Query, limit: usize) -> Result<Logs>;

    fn log_groups(&self, query: &Query, limit: usize) -> Result<LogGroups>;

    fn spans(&self, query: &Query, limit: usize) -> Result<Spans>;

    fn traces(&self, query: &Query, limit: usize) -> Result<Traces>;

    fn trace(&self, id: TraceId, limit: usize) -> Result<Option<Trace>>;

    fn metrics(&self, query: &Query, resolution: Resolution, limit: usize) -> Result<MetricList>;

    fn metric(&self, filter: &MetricFilter, limit: usize) -> Result<MetricSeries>;

    fn services(&self, step_ns: i64, limit: usize) -> Result<Services>;

    fn service(&self, service: &str, step_ns: i64, limit: usize) -> Result<Service>;

    fn operation(
        &self,
        service: &str,
        name: &str,
        kind: SpanKind,
        step_ns: i64,
    ) -> Result<OperationDetail>;

    fn calls(&self, service: &str, step_ns: i64, limit: usize) -> Result<Calls>;

    fn call(
        &self,
        service: &str,
        target: &TargetKey,
        summary: &str,
        kind: SpanKind,
        step_ns: i64,
    ) -> Result<CallDetail>;

    fn attributes(&self, signal: Signal) -> Result<AttributeKeys>;

    fn sql(&self, sql: &str, limit: usize) -> Result<SqlResult>;

    fn explain(&self, query: &Query) -> Result<Vec<String>>;
}
