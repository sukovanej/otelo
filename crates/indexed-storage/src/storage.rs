use std::collections::BTreeSet;
use std::time::Duration;

use otelo_query::{Catalog, Query, Signal};

use crate::query::{
    AttributeKeys, CallDetail, Calls, LogGroups, Logs, MetricFilter, MetricList, MetricSeries,
    OperationDetail, Resolution, Service, Services, Spans, TargetKey, Trace, Traces,
};
use crate::{IndexedAttribute, Result, SpanKind, TimeRange, TraceId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageSize {
    pub telemetry_bytes: u64,
    pub state_bytes: u64,
}

pub trait Storage: Send + Sync {
    fn oldest_retained_at(&self, signal: Signal) -> i64;

    fn size(&self) -> Result<StorageSize>;

    fn open_range(&self, range: TimeRange, time_limit: Duration) -> Result<Box<dyn RangeQueries>>;

    fn indexed_attributes(&self) -> BTreeSet<IndexedAttribute>;

    fn add_index(&self, attribute: &IndexedAttribute) -> Result<()>;

    fn remove_index(&self, attribute: &IndexedAttribute) -> Result<bool>;
}

pub trait RangeQueries: Catalog {
    fn list_logs(&self, query: &Query, limit: usize) -> Result<Logs>;

    fn list_log_groups(&self, query: &Query, limit: usize) -> Result<LogGroups>;

    fn list_spans(&self, query: &Query, limit: usize) -> Result<Spans>;

    fn list_traces(&self, query: &Query, limit: usize) -> Result<Traces>;

    fn get_trace(&self, trace_id: TraceId, limit: usize) -> Result<Option<Trace>>;

    fn list_metrics(
        &self,
        query: &Query,
        resolution: Resolution,
        limit: usize,
    ) -> Result<MetricList>;

    fn get_metric_series(&self, filter: &MetricFilter, limit: usize) -> Result<MetricSeries>;

    fn list_services(&self, step_ns: i64, limit: usize) -> Result<Services>;

    fn get_service(&self, service: &str, step_ns: i64, limit: usize) -> Result<Service>;

    fn get_operation(
        &self,
        service: &str,
        name: &str,
        kind: SpanKind,
        step_ns: i64,
    ) -> Result<OperationDetail>;

    fn list_calls(&self, service: &str, step_ns: i64, limit: usize) -> Result<Calls>;

    fn get_call(
        &self,
        service: &str,
        target: &TargetKey,
        summary: &str,
        kind: SpanKind,
        step_ns: i64,
    ) -> Result<CallDetail>;

    fn list_attribute_keys(&self, signal: Signal) -> Result<AttributeKeys>;

    fn explain_query(&self, query: &Query) -> Result<Vec<String>>;
}
