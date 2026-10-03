import { useNavigate } from "@solidjs/router";
import { createMemo, Errored, For, Show } from "solid-js";

import {
  getSpanGroups,
  getSpans,
  type SpanGroups,
  toQueryString,
  type TraceSpan,
} from "@otelo/api";
import { Callout, CloseButton, EmptyMessage, Modal } from "@otelo/ui";
import { ChartPanel, formatValue, Panel, Stat } from "@otelo/viz";

import { link, pageContent } from "../../classes";
import FetchErrorBoundary from "../../FetchErrorBoundary";
import SpanList from "../../traces/SpanList";
import SpanTitle from "../../traces/SpanTitle";
import { toTimeFrame } from "../frame";
import { createRangeFetch, type RangeState } from "../range";
import { PERCENTILES, toCountSeries, toErrorRateSeries, toLatencySeries } from "../series";
import type { SpanGroupingKind } from "../span-grouping";
import { measureSeconds, toRate, toShare } from "../stats";

const LISTED_SPAN_LIMIT = 50;

const GROUPING_LABELS: Record<SpanGroupingKind, GroupingLabels> = {
  route: { title: "Route", count: "Requests", noun: "route" },
  query: { title: "Query", count: "Queries", noun: "query" },
};

interface GroupingLabels {
  readonly title: string;
  readonly count: string;
  readonly noun: string;
}

interface SpanGroupResult {
  readonly groups: SpanGroups;
  readonly spans: ReadonlyArray<TraceSpan>;
}

interface ServicePageSpanGroupModalProps {
  readonly kind: SpanGroupingKind;
  readonly filter: string;
  readonly range: RangeState;
  readonly onClose: () => void;
}

export default function ServicePageSpanGroupModal(props: ServicePageSpanGroupModalProps) {
  const navigate = useNavigate();
  const labels = () => GROUPING_LABELS[props.kind];
  const fetched = createRangeFetch(
    props.range,
    "span-group",
    () => ({ q: props.filter }),
    async ({ q, since, until }, signal): Promise<SpanGroupResult> => {
      const [groups, list] = await Promise.all([
        getSpanGroups({ q, since, until }, signal),
        getSpans({ q, limit: LISTED_SPAN_LIMIT, since, until }, signal),
      ]);
      return { groups, spans: list.spans };
    },
  );

  const groups = () => fetched.data()?.groups;
  const group = () => groups()?.groups[0];
  const titleName = () => group()?.name ?? "";
  const titleAttributes = () => group()?.attributes ?? {};
  const timeFrame = createMemo(() => {
    const answer = groups();
    return answer && toTimeFrame(answer);
  });
  const steps = createMemo(() => groups()?.buckets.map((bucket) => bucket.spans) ?? []);
  const countSeries = createMemo(() => toCountSeries(steps()));
  const latencySeries = createMemo(() => toLatencySeries(steps()));
  const errorRateSeries = createMemo(() => toErrorRateSeries(steps()));
  const countTrend = createMemo(() => steps().map((step) => step.count));
  const stats = () => groups()?.spans;
  const zoomRangeTo = (start: number, end: number) =>
    props.range.setRange(new Date(start).toISOString(), new Date(end).toISOString());
  const tracesHref = () =>
    `/traces${toQueryString({ view: "spans", q: props.filter, since: props.range.since(), until: props.range.until() || undefined })}`;

  return (
    <Modal label={labels().title} onClose={props.onClose}>
      <div class="relative z-20 flex shrink-0 items-center gap-3 bg-surface px-4 py-2.5 shadow-(--raised)">
        <h1 class="m-0 min-w-0 max-w-[60ch] font-mono text-md font-semibold">
          <Errored fallback={labels().title}>
            <Show when={group()} fallback={labels().title}>
              <SpanTitle variant="group" name={titleName()} attributes={titleAttributes()} />
            </Show>
          </Errored>
        </h1>
        <span class="flex-1" />
        <Show when={fetched.loading()}>
          <span class="shrink-0 whitespace-nowrap text-muted" aria-live="polite">
            Loading…
          </span>
        </Show>
        <a
          href={tracesHref()}
          class={`shrink-0 whitespace-nowrap ${link}`}
          title={`Every span of the ${labels().noun} on the traces page`}
        >
          Open in Traces
        </a>
        <CloseButton onClose={props.onClose} />
      </div>

      <div class={`min-h-0 flex-1 bg-page ${pageContent}`}>
        <Show when={fetched.errorMessage()}>
          {(errorMessage) => (
            <div class="mb-3">
              <Callout tone="error">{errorMessage()}</Callout>
            </div>
          )}
        </Show>
        <FetchErrorBoundary>
          <div class="flex flex-col gap-4">
            <div class="grid grid-cols-[repeat(auto-fit,minmax(170px,1fr))] gap-3">
              <Stat
                label={labels().count}
                value={stats()?.count}
                unit="count"
                detail={formatValue(
                  toRate(
                    stats()?.count ?? 0,
                    measureSeconds(groups()?.start_at ?? "", groups()?.end_at ?? ""),
                  ),
                  "rate",
                )}
                trend={countTrend()}
              />
              <Stat
                label="Error rate"
                value={toShare(stats()?.errors ?? 0, stats()?.count ?? 0)}
                unit="ratio"
                tone={(stats()?.errors ?? 0) > 0 ? "error" : undefined}
                detail={`${(stats()?.errors ?? 0).toLocaleString()} failed`}
                trend={errorRateSeries()[0]?.values}
                trendColor="error"
              />
              <For each={PERCENTILES}>
                {(percentile, index) => (
                  <Stat
                    label={`${percentile.toUpperCase()} latency`}
                    value={stats()?.latency?.[percentile]}
                    unit="duration"
                    trend={latencySeries()[index()]?.values}
                    trendColor={percentile}
                  />
                )}
              </For>
            </div>

            <Show when={timeFrame()}>
              {(frame) => (
                <div class="grid grid-cols-1 gap-4 xl:grid-cols-3">
                  <ChartPanel
                    title={labels().count}
                    description="By step"
                    kind="bar"
                    unit="count"
                    frame={frame()}
                    series={countSeries()}
                    loading={fetched.loading()}
                    onZoom={zoomRangeTo}
                  />
                  <ChartPanel
                    title="Latency"
                    description="Percentiles of the durations"
                    kind="line"
                    unit="duration"
                    frame={frame()}
                    series={latencySeries()}
                    loading={fetched.loading()}
                    onZoom={zoomRangeTo}
                    emptyMessage={`No ${labels().count.toLowerCase()} in this range`}
                  />
                  <ChartPanel
                    title="Error rate"
                    description="The share that failed"
                    kind="area"
                    unit="ratio"
                    frame={frame()}
                    series={errorRateSeries()}
                    loading={fetched.loading()}
                    onZoom={zoomRangeTo}
                    emptyMessage={`No ${labels().count.toLowerCase()} in this range`}
                  />
                </div>
              )}
            </Show>

            <Panel
              title="Spans"
              description={`The newest ${LISTED_SPAN_LIMIT} spans of the ${labels().noun}; a span opens its trace`}
              flush
            >
              <Show when={fetched.data()}>
                {(result) => (
                  <Show
                    when={result().spans.length > 0}
                    fallback={
                      <EmptyMessage>The {labels().noun} has no spans in this range.</EmptyMessage>
                    }
                  >
                    <SpanList
                      spans={result().spans}
                      selectedKey={undefined}
                      onSelect={(span) => {
                        if (span)
                          navigate(
                            `/traces/${span.trace_id}${toQueryString({ span: span.span_id })}`,
                          );
                      }}
                    />
                  </Show>
                )}
              </Show>
            </Panel>
          </div>
        </FetchErrorBoundary>
      </div>
    </Modal>
  );
}
