import type {
  Api,
  LogGroup,
  LogGroups,
  LogLine,
  Logs,
  MetricList,
  MetricSeries,
  Service,
  ServiceSummary,
  SpanGroup,
  SpanGroups,
  Spans,
  SpanStats,
  Trace,
  Traces,
  TraceSpan,
  TraceSummary,
} from "@otelo/api";

const RANGE_START = "2026-10-01T09:00:00Z";

const RANGE_END = "2026-10-01T10:00:00Z";

const ERROR_SEVERITY = 17;

type ServicesBody = Awaited<ReturnType<Api["getServices"]>>;

type DashboardList = Awaited<ReturnType<Api["listDashboards"]>>;

export function toTraceSummary(traceId: string, durationNs: number): TraceSummary {
  return {
    trace_id: traceId,
    started_at: "2026-10-01T10:00:00Z",
    service: "api",
    name: "GET /orders",
    kind: 2,
    spans: 3,
    duration_ns: durationNs,
    error: false,
    attributes: {},
    resource: {},
  };
}

export function toTraces(traces: ReadonlyArray<TraceSummary>): Traces {
  return { traces: [...traces], next: null, unindexed: [] };
}

export function toLogLine(body: string, service: string, severity: number): LogLine {
  return {
    logged_at: "2026-10-01T10:00:00Z",
    service,
    severity,
    body,
    trace_id: null,
    span_id: null,
    attributes: {},
    resource: {},
  };
}

export function toLogs(lines: ReadonlyArray<LogLine>): Logs {
  return { logs: [...lines], next: null, unindexed: [] };
}

export function toTraceSpan(
  spanId: string,
  parentSpanId: string | null,
  startMs: number,
  durationMs: number,
): TraceSpan {
  return {
    trace_id: "t1",
    span_id: spanId,
    parent_span_id: parentSpanId,
    name: `step ${spanId}`,
    service: "api",
    kind: 1,
    status: 0,
    started_at: new Date(Date.UTC(2026, 9, 1, 10) + startMs).toISOString(),
    duration_ns: durationMs * 1e6,
    attributes: {},
    resource: {},
    events: [],
  };
}

export function toTrace(spans: ReadonlyArray<TraceSpan>): Trace {
  return { trace_id: "t1", spans: [...spans], logs: [], truncated: false };
}

export function toSpans(spans: ReadonlyArray<TraceSpan>): Spans {
  return { spans: [...spans], next: null, unindexed: [] };
}

export function toSpanStats(count: number): SpanStats {
  return {
    count,
    errors: 0,
    latency: { p50: 2e6, p95: 8e6, p99: 20e6 },
    total_ns: count * 3e6,
  };
}

export function toServiceSummary(service: string): ServiceSummary {
  return {
    service,
    resource: {},
    stats: { requests: toSpanStats(40), spans: 120, logs: 30, error_logs: 2 },
    buckets: [{ start_at: RANGE_START, requests: toSpanStats(40), logs: 30, error_logs: 2 }],
  };
}

export function toServices(names: ReadonlyArray<string>): ServicesBody {
  return {
    services: names.map(toServiceSummary),
    start_at: RANGE_START,
    end_at: RANGE_END,
    step_ns: 3600e9,
    truncated: false,
  };
}

export function toService(service: string): Service {
  return {
    ...toServiceSummary(service),
    start_at: RANGE_START,
    end_at: RANGE_END,
    step_ns: 3600e9,
  };
}

export function toRouteGroup(method: string, route: string): SpanGroup {
  return {
    name: `${method} ${route}`,
    attributes: {},
    values: { "http.request.method": method, "http.route": route },
    spans: toSpanStats(40),
    buckets: null,
  };
}

export function toSpanGroups(groups: ReadonlyArray<SpanGroup>): SpanGroups {
  return {
    groups: [...groups],
    spans: toSpanStats(40 * groups.length),
    buckets: [],
    start_at: RANGE_START,
    end_at: RANGE_END,
    step_ns: 3600e9,
    truncated: false,
    unindexed: [],
  };
}

export function toLogGroup(template: string): LogGroup {
  return {
    template,
    count: 3,
    first_at: RANGE_START,
    last_at: RANGE_END,
    samples: [template],
    services: ["api"],
    severity: ERROR_SEVERITY,
  };
}

export function toLogGroups(groups: ReadonlyArray<LogGroup>): LogGroups {
  return { groups: [...groups], partial: false, scanned: 3, truncated: false, unindexed: [] };
}

export function toMetricList(names: ReadonlyArray<string>): MetricList {
  return {
    series: names.map((name) => ({
      kind: "gauge",
      name,
      service: "api",
      unit: "By",
      attributes: {},
      resource: {},
    })),
    truncated: false,
  };
}

export function toEmptyMetricSeries(name: string): MetricSeries {
  return {
    name,
    start_at: RANGE_START,
    end_at: RANGE_END,
    step_ns: 60e9,
    resolution: "raw",
    groups: [],
    truncated: false,
  };
}

export function toDashboardList(names: ReadonlyArray<string>): DashboardList {
  return {
    dashboards: names.map((name, index) => ({
      id: index + 1,
      name,
      description: "",
      created_at: RANGE_START,
      updated_at: RANGE_START,
      widget_count: 0,
    })),
  };
}
