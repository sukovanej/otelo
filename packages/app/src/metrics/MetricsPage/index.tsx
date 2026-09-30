import { type Params, useParams } from "@solidjs/router";
import { createMemo, Show } from "solid-js";

import { getMetrics, type MetricList, toQueryString } from "@otelo/api";
import { Callout, EmptyMessage } from "@otelo/ui";

import { pageContent } from "../../classes";
import { formatCount } from "../../count";
import { API_MAX_ROWS, createListState, DEFAULT_SINCE, usePageKeys } from "../../list";
import QueryBar from "../../QueryBar";
import { summarizeMetricNames } from "../metric";
import MetricsPageMetric from "./metrics-page-metric";
import MetricsPageNameList from "./metrics-page-name-list";

const VIEWS = [{ value: "names", label: "Names" }] as const;

interface NamesResult {
  readonly view: "names";
  readonly body: MetricList;
}

interface MetricPathParams extends Params {
  readonly name?: string;
}

export default function MetricsPage() {
  const list = createListState<"names", NamesResult>({
    views: ["names"],
    firstLimits: { names: API_MAX_ROWS },
    fetch: async (key, signal) => ({ view: "names", body: await getMetrics(key, signal) }),
  });
  const params = useParams<MetricPathParams>();
  const openName = () => params.name && decodePathSegment(params.name);

  const allSeries = () => list.shownResult()?.body.series ?? [];
  const names = createMemo(() => summarizeMetricNames(allSeries()));
  const seriesOfOpenMetric = createMemo(() =>
    allSeries().filter((series) => series.name === openName()),
  );
  const toMetricHref = (name: string) =>
    `/metrics/${encodeURIComponent(name)}${toQueryString({
      q: list.query() || undefined,
      since: list.since() === DEFAULT_SINCE ? undefined : list.since(),
      until: list.until() || undefined,
      live: list.live() ? "1" : undefined,
    })}`;

  let queryInput: HTMLInputElement | undefined;
  usePageKeys({ queryInput: () => queryInput });

  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <QueryBar
        list={list}
        signal="metrics"
        placeholder='name ~ "memory" service = "api" system.memory.state = "used"'
        views={VIEWS}
        ref={(el) => (queryInput = el)}
      >
        <Show when={list.shownResult()}>
          {(result) => (
            <>
              {formatCount(names().length, "metric")} with{" "}
              {result().body.series.length.toLocaleString()} series
              {result().body.truncated ? "; more series match" : ""}
            </>
          )}
        </Show>
      </QueryBar>

      <div class="flex min-h-0 flex-1">
        <MetricsPageNameList
          names={names()}
          openName={openName()}
          loaded={list.shownResult() !== undefined}
          toHref={toMetricHref}
        />
        <div class={`min-w-0 flex-1 ${pageContent}`}>
          <Show when={list.fetched.errorMessage()}>
            {(errorMessage) => (
              <div class="mb-3">
                <Callout tone="error">{errorMessage()}</Callout>
              </div>
            )}
          </Show>
          <Show
            when={openName()}
            fallback={<EmptyMessage>Pick a metric on the left to chart it.</EmptyMessage>}
          >
            {(name) => (
              <MetricsPageMetric
                name={name()}
                query={list.query()}
                range={list}
                seriesOfMetric={seriesOfOpenMetric()}
              />
            )}
          </Show>
        </div>
      </div>
    </div>
  );
}

function decodePathSegment(segment: string): string {
  try {
    return decodeURIComponent(segment);
  } catch {
    return segment;
  }
}
