import { type SearchParams, useSearchParams } from "@solidjs/router";
import { createMemo, For, latest, Show } from "solid-js";

import { getMetricSeries, type MetricSeries, type SeriesInfo } from "@otelo/api";
import { Callout, CheckboxMenu, EmptyMessage, Select } from "@otelo/ui";
import { ChartPanel, formatValue } from "@otelo/viz";

import FetchErrorBoundary from "../../FetchErrorBoundary";
import { createRangeFetch, type RangeState } from "../../services/range";
import { listGroupingSections, toMetricCharts, toMetricFrame } from "../metric";

const TOP_OPTIONS = [
  { value: "", label: "Every group" },
  { value: "3", label: "3" },
  { value: "5", label: "5" },
  { value: "10", label: "10" },
] as const;

const RESOLUTION_NAMES: Record<MetricSeries["resolution"], string> = {
  raw: "the raw points",
  "1m": "the summaries by the minute",
  "1h": "the summaries by the hour",
};

type TopValue = (typeof TOP_OPTIONS)[number]["value"];

type MetricRange = Pick<RangeState, "since" | "until" | "live" | "setRange">;

interface GroupingSearchParams extends SearchParams {
  readonly by?: string;
  readonly top?: string;
}

interface MetricsPageMetricProps {
  readonly name: string;
  readonly query: string;
  readonly range: MetricRange;
  readonly seriesOfMetric: ReadonlyArray<SeriesInfo>;
}

export default function MetricsPageMetric(props: MetricsPageMetricProps) {
  const [params, setParams] = useSearchParams<GroupingSearchParams>();
  const by = createMemo(() => (params.by ?? "").split(",").filter((name) => name !== ""));
  const top = (): TopValue =>
    TOP_OPTIONS.find((option) => option.value === params.top)?.value ?? "";

  const fetched = createRangeFetch(
    props.range,
    "metric-series",
    () => ({ name: props.name, q: props.query.trim(), by: by().join(","), top: top() }),
    (key, signal) =>
      getMetricSeries(
        key.name,
        {
          q: key.q,
          since: key.since,
          until: key.until,
          by: key.by,
          ...(key.top === "" ? {} : { top: Number(key.top) }),
        },
        signal,
      ),
  );
  const metric = () => {
    const answer = fetched.data();
    return answer?.name === props.name ? answer : undefined;
  };
  const frame = createMemo(() => {
    const answer = metric();
    return answer && toMetricFrame(answer);
  });
  const charts = createMemo(() => {
    const answer = metric();
    const answerFrame = frame();
    return answer && answerFrame ? toMetricCharts(answer, answerFrame, by()) : [];
  });
  const zoomRangeTo = (startMs: number, endMs: number) =>
    props.range.setRange(new Date(startMs).toISOString(), new Date(endMs).toISOString());

  return (
    <div class="flex flex-col gap-3">
      <div class="flex flex-wrap items-center gap-3">
        <h1 class="m-0 min-w-0 flex-1 truncate font-mono text-base font-semibold">{props.name}</h1>
        <CheckboxMenu
          label="Group by"
          placeholder="nothing"
          sections={listGroupingSections(props.seriesOfMetric, latest(by))}
          checked={latest(by)}
          onChange={(checked) => setParams({ by: checked.join(",") || undefined })}
        />
        <Select
          label="Top"
          options={TOP_OPTIONS}
          value={latest(top)}
          onChange={(value) => setParams({ top: value || undefined })}
        />
      </div>

      <Show when={fetched.errorMessage()}>
        {(errorMessage) => <Callout tone="error">{errorMessage()}</Callout>}
      </Show>

      <FetchErrorBoundary>
        <Show when={metric()}>
          {(answer) => (
            <>
              <div class="-mt-2 text-muted">
                Steps of {formatValue(answer().step_ns, "duration")} from{" "}
                {RESOLUTION_NAMES[answer().resolution]}
              </div>
              <Show when={answer().truncated}>
                <Callout tone="hint">
                  More series match than the chart shows. Group them, keep the top ones, or narrow
                  the query.
                </Callout>
              </Show>
              <Show
                when={frame() && charts().length > 0 && frame()}
                fallback={
                  <EmptyMessage>
                    No series of this metric in this range match the query.
                  </EmptyMessage>
                }
              >
                {(answerFrame) => (
                  <For each={charts()} keyed={false}>
                    {(chart) => (
                      <ChartPanel
                        title={chart().title}
                        description={chart().description}
                        frame={answerFrame()}
                        series={chart().series}
                        kind="line"
                        unit={chart().unit}
                        loading={fetched.loading()}
                        onZoom={zoomRangeTo}
                      />
                    )}
                  </For>
                )}
              </Show>
            </>
          )}
        </Show>
      </FetchErrorBoundary>
    </div>
  );
}
