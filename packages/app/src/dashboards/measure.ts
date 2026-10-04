import type {
  Attributes,
  LogCounts,
  MetricAggregation,
  MetricSeries,
  SeriesGroup,
  SpanBucket,
  SpanGroups,
  SpanMeasure,
  SpanStats,
} from "@otelo/api";
import type { SeriesColor, TimeFrame, TimeSeries, Unit } from "@otelo/viz";

import {
  formatAttributeValue,
  labelSeriesGroups,
  type MetricKindName,
  pickChartUnit,
  type ReadBucketValue,
  readGroupValues,
  readPercentile,
  readRatePerSecond,
  toMetricFrame,
} from "../metrics/metric";
import { toTimeFrame } from "../services/frame";
import { measureSeconds } from "../services/stats";

const SPAN_MEASURE_UNITS: Record<SpanMeasure, Unit> = {
  count: "count",
  rate: "rate",
  errors: "count",
  error_rate: "ratio",
  p50: "duration",
  p95: "duration",
  p99: "duration",
};

const SPAN_MEASURE_COLORS: Record<SpanMeasure, SeriesColor | undefined> = {
  count: undefined,
  rate: undefined,
  errors: "error",
  error_rate: "error",
  p50: "p50",
  p95: "p95",
  p99: "p99",
};

const SPAN_MEASURE_READERS: Record<SpanMeasure, ReadSpanMeasure> = {
  count: (stats) => stats.count,
  rate: (stats, seconds) => (seconds > 0 ? stats.count / seconds : null),
  errors: (stats) => stats.errors,
  error_rate: (stats) => (stats.count > 0 ? stats.errors / stats.count : null),
  p50: (stats) => stats.latency?.p50 ?? null,
  p95: (stats) => stats.latency?.p95 ?? null,
  p99: (stats) => stats.latency?.p99 ?? null,
};

const METRIC_BUCKET_READERS: Record<MetricAggregation, ReadBucketValue> = {
  avg: (bucket) => bucket.avg,
  min: (bucket) => bucket.min,
  max: (bucket) => bucket.max,
  last: (bucket) => bucket.last,
  rate: readRatePerSecond,
  p50: readPercentile("p50"),
  p90: readPercentile("p90"),
  p99: readPercentile("p99"),
};

const METRIC_RANGE_AGGREGATORS: Record<MetricAggregation, AggregateValues> = {
  avg: averageValues,
  min: (values) => (values.length > 0 ? Math.min(...values) : null),
  max: (values) => (values.length > 0 ? Math.max(...values) : null),
  last: (values) => values.at(-1) ?? null,
  rate: averageValues,
  p50: averageValues,
  p90: averageValues,
  p99: averageValues,
};

const METRIC_AGGREGATION_COLORS: Record<MetricAggregation, SeriesColor | undefined> = {
  avg: undefined,
  min: undefined,
  max: undefined,
  last: undefined,
  rate: undefined,
  p50: "p50",
  p90: "p90",
  p99: "p99",
};

export interface MeasuredQuery {
  readonly frame: TimeFrame;
  readonly unit: Unit;
  readonly groups: ReadonlyArray<MeasuredGroup>;
  readonly truncated: boolean;
}

interface MeasuredGroup {
  readonly label: string;
  readonly values: ReadonlyArray<number | null>;
  readonly total: number | null;
  readonly color?: SeriesColor;
}

export interface MeasuredChart {
  readonly frame: TimeFrame;
  readonly unit: Unit;
  readonly series: ReadonlyArray<TimeSeries>;
  readonly truncated: boolean;
  // The queries left out because their numbers have another unit than the
  // first query's, which sets the axis.
  readonly otherUnitQueryCount: number;
}

// A metric grouped with `top` folds the groups past the top ones into one: a
// chart draws it, and a ranking leaves it out.
export type FoldedGroup = "kept" | "dropped";

type ReadSpanMeasure = (stats: SpanStats, seconds: number) => number | null;

type AggregateValues = (values: ReadonlyArray<number>) => number | null;

export function measureSpanGroups(
  answer: SpanGroups,
  measure: SpanMeasure,
  by: ReadonlyArray<string>,
  label: string,
): MeasuredQuery {
  const frame = toTimeFrame(answer);
  const stepSeconds = answer.step_ns / 1e9;
  const rangeSeconds = measureSeconds(answer.start_at, answer.end_at);
  const readMeasure = SPAN_MEASURE_READERS[measure];
  const measureSteps = (buckets: ReadonlyArray<SpanBucket>) =>
    buckets.map((bucket) => readMeasure(bucket.spans, stepSeconds));
  if (by.length === 0) {
    const color = SPAN_MEASURE_COLORS[measure];
    return {
      frame,
      unit: SPAN_MEASURE_UNITS[measure],
      groups: [
        {
          label,
          values: measureSteps(answer.buckets),
          total: readMeasure(answer.spans, rangeSeconds),
          ...(color ? { color } : {}),
        },
      ],
      truncated: false,
    };
  }
  return {
    frame,
    unit: SPAN_MEASURE_UNITS[measure],
    groups: answer.groups.map((group) => ({
      label: labelGroupValues(group.values, by),
      values: measureSteps(group.buckets ?? []),
      total: readMeasure(group.spans, rangeSeconds),
    })),
    truncated: answer.truncated,
  };
}

export function measureLogCounts(
  answer: LogCounts,
  by: ReadonlyArray<string>,
  label: string,
): MeasuredQuery {
  const frame = toTimeFrame(answer);
  return {
    frame,
    unit: "count",
    groups:
      by.length === 0
        ? [{ label, values: countSteps(answer.buckets), total: answer.count }]
        : answer.groups.map((group) => ({
            label: labelGroupValues(group.values, by),
            values: countSteps(group.buckets),
            total: group.count,
          })),
    truncated: answer.truncated,
  };
}

export function measureMetricSeries(
  answer: MetricSeries,
  aggregation: MetricAggregation,
  by: ReadonlyArray<string>,
  label: string,
  foldedGroup: FoldedGroup,
): MeasuredQuery | undefined {
  const [firstGroup] = answer.groups;
  if (!firstGroup) return undefined;
  const frame = toMetricFrame(answer);
  const chartUnit = pickChartUnit(firstGroup.unit, pickKindOfAggregation(firstGroup, aggregation));
  const readValue = METRIC_BUCKET_READERS[aggregation];
  const aggregateOverRange = METRIC_RANGE_AGGREGATORS[aggregation];
  const measureGroup = (values: ReadonlyArray<number | null>) =>
    aggregateOverRange(values.filter((value) => value !== null));
  const valuesOfGroups = answer.groups.map((group) =>
    readGroupValues(group, frame, readValue, chartUnit.factor),
  );
  const color = METRIC_AGGREGATION_COLORS[aggregation];
  if (by.length === 0) {
    const values = combineSeriesValues(valuesOfGroups, firstGroup.kind);
    return {
      frame,
      unit: chartUnit.unit,
      groups: [{ label, values, total: measureGroup(values), ...(color ? { color } : {}) }],
      truncated: answer.truncated,
    };
  }
  const labels = labelSeriesGroups(answer.groups, by);
  const groups = answer.groups.flatMap((group, index) => {
    const isFolded = group.key.type === "other";
    if (isFolded && foldedGroup === "dropped") return [];
    const values = valuesOfGroups[index] ?? [];
    return [
      {
        label: labels[index] ?? "",
        values,
        total: measureGroup(values),
        ...(isFolded ? { color: "muted" as const } : {}),
      },
    ];
  });
  return {
    frame,
    unit: chartUnit.unit,
    groups,
    truncated: answer.truncated || groups.length < answer.groups.length,
  };
}

export function combineMeasuredQueries(
  measured: ReadonlyArray<MeasuredQuery>,
): MeasuredChart | undefined {
  const [first] = measured;
  if (!first) return undefined;
  const sameUnit = measured.filter((query) => query.unit === first.unit);
  return {
    frame: first.frame,
    unit: first.unit,
    series: sameUnit.flatMap((query) =>
      query.groups.map((group) => ({
        label: group.label,
        values: alignValuesToFrame(group.values, query.frame, first.frame),
        ...(group.color ? { color: group.color } : {}),
      })),
    ),
    truncated: sameUnit.some((query) => query.truncated),
    otherUnitQueryCount: measured.length - sameUnit.length,
  };
}

function countSteps(buckets: LogCounts["buckets"]): number[] {
  return buckets.map((bucket) => bucket.count);
}

function labelGroupValues(values: Attributes, by: ReadonlyArray<string>): string {
  return by.map((name) => formatAttributeValue(values[name])).join(" · ");
}

// The series of a metric combine as the daemon combines a group: a gauge takes
// their average, a histogram the largest of their percentiles, and the other
// kinds their sum.
function combineSeriesValues(
  valuesOfSeries: ReadonlyArray<ReadonlyArray<number | null>>,
  kind: MetricKindName,
): (number | null)[] {
  const stepCount = Math.max(0, ...valuesOfSeries.map((values) => values.length));
  return Array.from({ length: stepCount }, (_, stepIndex) => {
    const values = valuesOfSeries
      .map((seriesValues) => seriesValues[stepIndex] ?? null)
      .filter((value) => value !== null);
    if (values.length === 0) return null;
    if (kind === "gauge") return averageValues(values);
    if (kind === "histogram") return Math.max(...values);
    return values.reduce((sum, value) => sum + value, 0);
  });
}

function pickKindOfAggregation(group: SeriesGroup, aggregation: MetricAggregation): MetricKindName {
  if (aggregation === "rate") return "counter";
  if (aggregation === "p50" || aggregation === "p90" || aggregation === "p99") return "histogram";
  return group.kind === "gauge" ? "gauge" : "updown";
}

function alignValuesToFrame(
  values: ReadonlyArray<number | null>,
  from: TimeFrame,
  to: TimeFrame,
): ReadonlyArray<number | null> {
  if (from === to) return values;
  const valuesByStartMs = new Map(
    from.bucketStartsMs.map((startMs, index) => [startMs, values[index] ?? null]),
  );
  return to.bucketStartsMs.map((startMs) => valuesByStartMs.get(startMs) ?? null);
}

function averageValues(values: ReadonlyArray<number>): number | null {
  return values.length > 0 ? values.reduce((sum, value) => sum + value, 0) / values.length : null;
}
