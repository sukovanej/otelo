import { createMemo, createSignal, Errored, latest, Show } from "solid-js";

import {
  completeQuery,
  getAttributeKeys,
  getMetrics,
  type GroupedQuery,
  type WidgetQuery,
} from "@otelo/api";
import { QueryInput, Select, type SelectOption, Tabs } from "@otelo/ui";

import { fieldLabel } from "../../classes";
import { createFetch, freezeDeeply } from "../../fetch";
import { highlightQuery } from "../../highlight";
import { listGroupingOptions, summarizeMetricNames } from "../../metrics/metric";
import { createRangeFetch, type RangeState } from "../../services/range";
import {
  changeQuerySignal,
  listCatalogGroupingOptions,
  METRIC_AGGREGATION_OPTIONS,
  type QuerySignal,
  SIGNAL_OPTIONS,
  SPAN_MEASURE_OPTIONS,
} from "../widget";

const FILTER_PLACEHOLDERS: Record<QuerySignal, string> = {
  spans: "kind = server service = api",
  logs: "level >= warn",
  metrics: "state = used",
};

interface WidgetEditorQueryProps {
  readonly grouped: GroupedQuery;
  readonly groupable: boolean;
  readonly range: Pick<RangeState, "since" | "until" | "live">;
  readonly onChange: (grouped: GroupedQuery) => void;
}

export default function WidgetEditorQuery(props: WidgetEditorQueryProps) {
  const query = () => props.grouped.query;
  const [draftFilter, setDraftFilter] = createSignal(() => props.grouped.query.filter);
  const changeQuery = (changed: WidgetQuery) =>
    props.onChange({ ...props.grouped, query: changed });
  const commitFilter = () => {
    if (draftFilter() !== query().filter) changeQuery({ ...query(), filter: draftFilter() });
  };
  const metricQuery = () => {
    const shownQuery = query();
    return shownQuery.signal === "metrics" ? shownQuery : undefined;
  };
  const spanQuery = () => {
    const shownQuery = query();
    return shownQuery.signal === "spans" ? shownQuery : undefined;
  };
  const fetchedMetrics = createRangeFetch(
    props.range,
    "dashboard-metric-names",
    () => ({ signal: query().signal }),
    ({ signal, since, until }, abortSignal) =>
      signal === "metrics"
        ? getMetrics({ since, until, limit: 10_000 }, abortSignal).then(freezeDeeply)
        : Promise.resolve(null),
  );
  const fetchedAttributeKeys = createFetch(
    "dashboard-attribute-keys",
    () => ({ signal: query().signal }),
    ({ signal }, abortSignal) => getAttributeKeys(signal, abortSignal).then(freezeDeeply),
  );
  const metricNameOptions = createMemo(() =>
    summarizeMetricNames(fetchedMetrics.data()?.series ?? []).map((metric) => ({
      value: metric.name,
      label: metric.name,
    })),
  );
  const groupingOptions = createMemo(() => {
    const metricName = metricQuery()?.name ?? "";
    const seriesOfMetric = (fetchedMetrics.data()?.series ?? []).filter(
      (series) => series.name === metricName,
    );
    return seriesOfMetric.length > 0
      ? listGroupingOptions(seriesOfMetric, props.grouped.by)
      : listCatalogGroupingOptions(query().signal, fetchedAttributeKeys.data(), props.grouped.by);
  });

  const drawGroupingSelect = (options: () => ReadonlyArray<SelectOption<string>>) => (
    <Select
      selection="multiple"
      label="Group by"
      placeholder="nothing"
      options={options()}
      values={props.grouped.by}
      typedValue={(text) => text}
      stretch
      onChange={(by) => props.onChange({ ...props.grouped, by })}
    />
  );

  return (
    <div class="flex flex-col gap-3">
      <Tabs
        label="Signal"
        options={SIGNAL_OPTIONS}
        value={query().signal}
        onChange={(signal) => props.onChange({ query: changeQuerySignal(query(), signal), by: [] })}
      />
      <Show when={metricQuery()}>
        {(metric) => {
          const drawMetricSelect = (options: () => ReadonlyArray<SelectOption<string>>) => (
            <Select
              selection="single"
              label="Metric"
              placeholder="Pick a metric"
              options={options()}
              value={metric().name}
              typedValue={(text) => text}
              stretch
              onChange={(name) => changeQuery({ ...metric(), name })}
            />
          );
          return (
            <Errored fallback={drawMetricSelect(() => [])}>
              {drawMetricSelect(metricNameOptions)}
            </Errored>
          );
        }}
      </Show>
      <div
        class="flex flex-col gap-1.5"
        onFocusOut={(e) => {
          const nextFocused = e.relatedTarget;
          if (!(nextFocused instanceof Node && e.currentTarget.contains(nextFocused))) {
            commitFilter();
          }
        }}
      >
        <span class={fieldLabel} aria-hidden="true">
          Filter
        </span>
        <QueryInput
          label="Filter"
          value={draftFilter()}
          highlight={highlightQuery}
          complete={(text, cursorInChars, abort) =>
            completeQuery(query().signal, text, cursorInChars, abort).then(
              (completions) => completions.suggestions,
            )
          }
          help={() => Promise.resolve(undefined)}
          onInput={setDraftFilter}
          onSubmit={commitFilter}
          placeholder={FILTER_PLACEHOLDERS[query().signal]}
        />
      </div>
      <Show when={spanQuery() ?? metricQuery() ?? props.groupable}>
        <div class="flex flex-wrap items-start gap-2">
          <Show when={spanQuery()}>
            {(spans) => (
              <Select
                selection="single"
                label="Measure"
                options={SPAN_MEASURE_OPTIONS}
                value={spans().measure}
                onChange={(measure) => changeQuery({ ...spans(), measure })}
              />
            )}
          </Show>
          <Show when={metricQuery()}>
            {(metric) => (
              <Select
                selection="single"
                label="Aggregation"
                options={METRIC_AGGREGATION_OPTIONS}
                value={metric().aggregation}
                onChange={(aggregation) => changeQuery({ ...metric(), aggregation })}
              />
            )}
          </Show>
          <Show when={props.groupable}>
            <div class="min-w-0 flex-1 basis-56">
              <Errored
                fallback={drawGroupingSelect(() =>
                  listCatalogGroupingOptions(query().signal, undefined, props.grouped.by),
                )}
              >
                {drawGroupingSelect(() => latest(groupingOptions))}
              </Errored>
            </div>
          </Show>
        </div>
      </Show>
    </div>
  );
}
