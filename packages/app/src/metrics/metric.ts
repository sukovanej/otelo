import type { Attributes, AttributeValue, MetricSeries, SeriesGroup, SeriesInfo } from "@otelo/api";
import type { SelectOption } from "@otelo/ui";
import type { TimeFrame, TimeSeries, Unit } from "@otelo/viz";

import { formatCount } from "../count";
import { writeAttributeField, writeResourceField } from "../query";
import { parseTime } from "../time";

// A value stands for the steps after it until a newer one comes, as a sample
// does in Prometheus, so an app that sends every minute draws a line and not
// dashes at a step of 30 seconds.
const VALUE_STALE_AFTER_MS = 5 * 60_000;

const PERCENTILES = ["p50", "p90", "p99"] as const;

const NANOS_PER_DURATION_UNIT: Readonly<Record<string, number>> = {
  ns: 1,
  us: 1e3,
  µs: 1e3,
  ms: 1e6,
  s: 1e9,
  min: 60e9,
  h: 3600e9,
  d: 86_400e9,
};

const BYTES_PER_BYTE_UNIT: Readonly<Record<string, number>> = {
  By: 1,
  KiBy: 1024,
  MiBy: 1024 ** 2,
  GiBy: 1024 ** 3,
  TiBy: 1024 ** 4,
  kBy: 1e3,
  KBy: 1e3,
  MBy: 1e6,
  GBy: 1e9,
  TBy: 1e12,
};

const KIND_TITLES: Record<MetricKindName, string> = {
  gauge: "Average of each step",
  updown: "Average of each step",
  counter: "Rate per second",
  histogram: "Percentiles",
};

export interface MetricName {
  readonly name: string;
  readonly kinds: ReadonlyArray<MetricKindName>;
  readonly units: ReadonlyArray<string>;
  readonly seriesCount: number;
}

export type MetricKindName = SeriesInfo["kind"];

type Percentile = (typeof PERCENTILES)[number];

export type ReadBucketValue = (bucket: SeriesGroup["buckets"][number]) => number | null;

interface ChartUnit {
  readonly unit: Unit;
  readonly factor: number;
}

interface MetricChart {
  readonly title: string;
  readonly description: string;
  readonly unit: Unit;
  readonly series: ReadonlyArray<TimeSeries>;
}

type GroupKey = SeriesGroup["key"];

type SeriesGroupKey = Extract<GroupKey, { readonly type: "series" }>;

export function summarizeMetricNames(series: ReadonlyArray<SeriesInfo>): MetricName[] {
  const namesByName = new Map<string, MetricName>();
  for (const oneSeries of series) {
    const known = namesByName.get(oneSeries.name);
    namesByName.set(oneSeries.name, {
      name: oneSeries.name,
      kinds: addUnique(known?.kinds ?? [], oneSeries.kind),
      units: addUnique(known?.units ?? [], oneSeries.unit),
      seriesCount: (known?.seriesCount ?? 0) + 1,
    });
  }
  return [...namesByName.values()];
}

export function listGroupingOptions(
  seriesOfMetric: ReadonlyArray<SeriesInfo>,
  checked: ReadonlyArray<string>,
): SelectOption<string>[] {
  const attributeOptions = listKeys(seriesOfMetric.map((series) => series.attributes))
    .filter((key) => !writeAttributeField(key).startsWith("`"))
    .map((key) => ({ value: writeAttributeField(key), label: key }));
  const resourceOptions = listKeys(seriesOfMetric.map((series) => series.resource)).flatMap(
    (key) => {
      const field = writeResourceField(key);
      const hasValuesThatDiffer =
        new Set(seriesOfMetric.map((series) => formatAttributeValue(series.resource[key]))).size >
        1;
      return field && (hasValuesThatDiffer || checked.includes(field))
        ? [{ value: field, label: field }]
        : [];
    },
  );
  const knownValues = new Set([
    "service",
    ...[...attributeOptions, ...resourceOptions].map((option) => option.value),
  ]);
  const unknownChecked = checked
    .filter((value) => !knownValues.has(value))
    .map((value) => ({ value, label: value }));
  return [
    ...[
      ...attributeOptions,
      ...unknownChecked.filter((unknown) => !unknown.value.startsWith("resource.")),
    ].map(({ value, label }) => ({ value, label, section: "Attributes" })),
    { value: "service", label: "service", section: "Service" },
    ...[
      ...resourceOptions,
      ...unknownChecked.filter((unknown) => unknown.value.startsWith("resource.")),
    ].map(({ value, label }) => ({ value, label, section: "Resource" })),
  ];
}

export function toMetricFrame(metric: MetricSeries): TimeFrame {
  const stepMs = metric.step_ns / 1e6;
  const startMs = parseTime(metric.start_at).getTime();
  const endMs = parseTime(metric.end_at).getTime();
  const firstBucketStartMs = Math.floor(startMs / stepMs) * stepMs;
  const bucketStartsMs = Array.from(
    { length: Math.max(0, Math.ceil((endMs - firstBucketStartMs) / stepMs)) },
    (_, index) => firstBucketStartMs + index * stepMs,
  );
  return { bucketStartsMs, stepMs, startMs, endMs };
}

export function toMetricCharts(
  metric: MetricSeries,
  frame: TimeFrame,
  by: ReadonlyArray<string>,
): MetricChart[] {
  const groupsByKindAndUnit = new Map<string, SeriesGroup[]>();
  for (const group of metric.groups) {
    const kindAndUnit = JSON.stringify([group.kind, group.unit]);
    groupsByKindAndUnit.set(kindAndUnit, [...(groupsByKindAndUnit.get(kindAndUnit) ?? []), group]);
  }
  return [...groupsByKindAndUnit.values()].flatMap((groups) => {
    const [firstGroup] = groups;
    if (!firstGroup) return [];
    const description = `${firstGroup.kind} in ${firstGroup.unit === "" ? "no unit" : firstGroup.unit}`;
    const chartUnit = pickChartUnit(firstGroup.unit, firstGroup.kind);
    const labels = labelSeriesGroups(groups, by);
    const toSeries = (group: SeriesGroup, index: number, values: (number | null)[]) =>
      withOtherColor(group.key, { label: labels[index] ?? "", values });

    if (firstGroup.kind !== "histogram") {
      return [
        {
          title: KIND_TITLES[firstGroup.kind],
          description,
          unit: chartUnit.unit,
          series: groups.map((group, index) =>
            toSeries(group, index, readAveragesOrRates(group, frame)),
          ),
        },
      ];
    }
    if (groups.length === 1) {
      return [
        {
          title: KIND_TITLES.histogram,
          description,
          unit: chartUnit.unit,
          series: PERCENTILES.map((percentile) => ({
            label: percentile.toUpperCase(),
            color: percentile,
            values: readGroupValues(
              firstGroup,
              frame,
              readPercentile(percentile),
              chartUnit.factor,
            ),
          })),
        },
      ];
    }
    return PERCENTILES.map((percentile) => ({
      title: `${percentile.toUpperCase()} of each group`,
      description,
      unit: chartUnit.unit,
      series: groups.map((group, index) =>
        toSeries(
          group,
          index,
          readGroupValues(group, frame, readPercentile(percentile), chartUnit.factor),
        ),
      ),
    }));
  });
}

export function readAveragesOrRates(group: SeriesGroup, frame: TimeFrame): (number | null)[] {
  const readValue: ReadBucketValue = group.kind === "counter" ? readRatePerSecond : readAverage;
  return readGroupValues(group, frame, readValue, pickChartUnit(group.unit, group.kind).factor);
}

export function labelSeriesGroups(
  groups: ReadonlyArray<SeriesGroup>,
  by: ReadonlyArray<string>,
): string[] {
  const seriesKeys = groups.flatMap((group) => (group.key.type === "series" ? [group.key] : []));
  const seriesLabels = labelSeriesKeys(seriesKeys);
  const labelsBySeriesKey = new Map(seriesKeys.map((key, index) => [key, seriesLabels[index]]));
  return groups.map(({ key }) => {
    if (key.type === "series") return labelsBySeriesKey.get(key) ?? key.service;
    if (key.type === "values") {
      return by.map((name) => formatAttributeValue(key.values[name])).join(" · ");
    }
    return formatCount(key.group_count, "other group");
  });
}

export function formatAttributeValue(value: AttributeValue | undefined): string {
  if (value === undefined) return "–";
  if (typeof value === "string") return value;
  if (value === null || typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}

export function pickChartUnit(unit: string, kind: MetricKindName): ChartUnit {
  const nanos = NANOS_PER_DURATION_UNIT[unit];
  const bytes = BYTES_PER_BYTE_UNIT[unit];
  const isCount = unit === "" || unit === "1" || /^\{.*\}$/.test(unit);
  if (kind === "counter") {
    if (bytes !== undefined) return { unit: "bytes-per-second", factor: bytes };
    if (nanos !== undefined) return { unit: "number", factor: nanos / 1e9 };
    return { unit: isCount ? "rate" : "number", factor: 1 };
  }
  if (nanos !== undefined) return { unit: "duration", factor: nanos };
  if (bytes !== undefined) return { unit: "bytes", factor: bytes };
  if (unit === "%") return { unit: "ratio", factor: 0.01 };
  if (unit === "1" && kind === "gauge") return { unit: "ratio", factor: 1 };
  return { unit: isCount ? "count" : "number", factor: 1 };
}

export function readRatePerSecond(bucket: SeriesGroup["buckets"][number]): number | null {
  return bucket.change.kind === "rate" ? bucket.change.per_second : null;
}

export function readPercentile(percentile: Percentile): ReadBucketValue {
  return (bucket) =>
    bucket.change.kind === "distribution"
      ? (bucket.change.percentiles?.[percentile] ?? null)
      : null;
}

export function readGroupValues(
  group: SeriesGroup,
  frame: TimeFrame,
  readValue: ReadBucketValue,
  factor: number,
): (number | null)[] {
  const bucketsByStartMs = new Map(
    group.buckets.map((bucket) => [parseTime(bucket.start_at).getTime(), bucket]),
  );
  const values = frame.bucketStartsMs.map((bucketStartMs) => {
    const bucket = bucketsByStartMs.get(bucketStartMs);
    const value = bucket ? readValue(bucket) : null;
    return value === null ? null : value * factor;
  });
  return carryValuesIntoEmptySteps(values, frame.stepMs);
}

function labelSeriesKeys(keys: ReadonlyArray<SeriesGroupKey>): string[] {
  const servicesDiffer = new Set(keys.map((key) => key.service)).size > 1;
  const attributeKeysThatDiffer = listKeysThatDiffer(keys.map((key) => key.attributes));
  const labels = keys.map((key) =>
    [
      ...(servicesDiffer ? [key.service] : []),
      ...attributeKeysThatDiffer.map((attributeKey) =>
        formatAttributeValue(key.attributes[attributeKey]),
      ),
    ].join(" · "),
  );
  const resourceKeysThatDiffer =
    new Set(labels).size < labels.length ? listKeysThatDiffer(keys.map((key) => key.resource)) : [];
  return keys.map((key, index) => {
    const label = [
      labels[index] ?? "",
      ...resourceKeysThatDiffer.map(
        (resourceKey) => `${resourceKey}=${formatAttributeValue(key.resource[resourceKey])}`,
      ),
    ]
      .filter((part) => part !== "")
      .join(" · ");
    return label === "" ? key.service : label;
  });
}

function listKeysThatDiffer(attributeSets: ReadonlyArray<Attributes>): string[] {
  return listKeys(attributeSets).filter(
    (key) =>
      new Set(attributeSets.map((attributes) => formatAttributeValue(attributes[key]))).size > 1,
  );
}

function listKeys(attributeSets: ReadonlyArray<Attributes>): string[] {
  return [...new Set(attributeSets.flatMap((attributes) => Object.keys(attributes)))].toSorted();
}

function addUnique<T>(values: ReadonlyArray<T>, value: T): ReadonlyArray<T> {
  return values.includes(value) ? values : [...values, value];
}

function readAverage(bucket: SeriesGroup["buckets"][number]): number | null {
  return bucket.avg;
}

function withOtherColor(key: GroupKey, series: TimeSeries): TimeSeries {
  return key.type === "other" ? { ...series, color: "muted" } : series;
}

function carryValuesIntoEmptySteps(
  values: ReadonlyArray<number | null>,
  stepMs: number,
): (number | null)[] {
  const carriedStepCount = Math.ceil(VALUE_STALE_AFTER_MS / stepMs) - 1;
  let lastValue: number | null = null;
  let emptyStepCount = 0;
  return values.map((value) => {
    if (value !== null) {
      lastValue = value;
      emptyStepCount = 0;
      return value;
    }
    emptyStepCount++;
    return emptyStepCount <= carriedStepCount ? lastValue : null;
  });
}
