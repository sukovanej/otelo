import { A, useNavigate } from "@solidjs/router";
import { createMemo, For, Show } from "solid-js";

import { getOperation, getSpans, search } from "@siner/api";
import { Callout } from "@siner/ui";
import { ChartPanel, formatValue, Panel, Stat } from "@siner/viz";

import { link, pageContent } from "../classes";
import { Empty } from "../ListFrame";
import Modal from "../Modal";
import { CloseButton } from "../Panel";
import { quote } from "../query";
import KindBadge from "../traces/KindBadge";
import { kindName } from "../traces/span";
import SpanList from "../traces/SpanList";
import SpanTitle from "../traces/SpanTitle";
import { timeFrame } from "./frame";
import { createRangeFetch, type Range } from "./range";
import { errorRateSeries, latencySeries, PERCENTILES, requestSeries } from "./series";
import { rate, seconds, share } from "./stats";

/** How many of the newest spans of the operation the modal lists. */
const SPANS = 50;

/**
 * The query that keeps the spans an operation counts: its spans in the
 * service by name and kind, and only the roots for a kind that does not
 * enter a service by itself.
 */
export function operationQuery(service: string, name: string, kind: number): string {
  const kindTerm = kind === 0 ? "" : ` kind = ${kindName(kind)}`;
  const root = kind === 2 || kind === 5 ? "" : " root = true";
  return `service = ${quote(service)} name = ${quote(name)}${kindTerm}${root}`;
}

/**
 * One operation of a service over the page it was opened from: its numbers
 * over the range, its requests and latency over time, and its newest spans,
 * which open their traces. It follows the range of the page, and dragging
 * across a chart zooms that range.
 */
export default function OperationModal(props: {
  service: string;
  name: string;
  kind: number;
  range: Range;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const query = () => operationQuery(props.service, props.name, props.kind);
  const fetched = createRangeFetch(
    props.range,
    () => ({ service: props.service, operation: props.name, kind: props.kind }),
    ({ service, ...params }, signal) => getOperation(service, params, signal),
  );
  const spans = createRangeFetch(props.range, () => ({ q: query(), limit: SPANS }), getSpans);

  const data = () => fetched.data();
  const frame = createMemo(() => {
    const operation = data();
    return operation && timeFrame(operation);
  });
  const requestChart = createMemo(() => requestSeries(data()?.buckets ?? []));
  const latencyChart = createMemo(() => latencySeries(data()?.buckets ?? []));
  const errorRateChart = createMemo(() => errorRateSeries(data()?.buckets ?? []));
  const requests = () => data()?.requests;
  const zoom = (start: number, end: number) =>
    props.range.setRange(new Date(start).toISOString(), new Date(end).toISOString());

  return (
    <Modal label={`Operation ${props.name}`} onClose={props.onClose}>
      <div class="relative z-20 flex shrink-0 items-center gap-3 bg-surface px-4 py-2.5 shadow-(--raised)">
        <h1 class="m-0 min-w-0 font-mono text-md font-semibold">
          <SpanTitle
            name={props.name}
            attributes={data()?.attributes ?? {}}
            error={false}
            status={false}
          />
        </h1>
        <KindBadge kind={props.kind} />
        <span class="flex-1" />
        <Show when={fetched.loading()}>
          <span class="text-muted" aria-live="polite">
            Loading…
          </span>
        </Show>
        <A
          href={`/traces${search({ view: "spans", q: query(), since: props.range.since(), until: props.range.until() || undefined })}`}
          class={link}
          title="Every span of the operation on the traces page"
        >
          Open in Traces
        </A>
        <CloseButton onClose={props.onClose} />
      </div>

      <div class={`min-h-0 flex-1 bg-page ${pageContent}`}>
        <Show when={fetched.error()}>
          {(error) => (
            <div class="mb-3">
              <Callout tone="error">{error()}</Callout>
            </div>
          )}
        </Show>
        <div class="flex flex-col gap-4">
          <div class="grid grid-cols-[repeat(auto-fit,minmax(170px,1fr))] gap-3">
            <Stat
              label="Requests"
              value={requests()?.count}
              unit="count"
              detail={formatValue(
                rate(requests()?.count ?? 0, seconds(data()?.since ?? "", data()?.until ?? "")),
                "rate",
              )}
              trend={data()?.buckets.map((b) => b.requests.count)}
            />
            <Stat
              label="Error rate"
              value={share(requests()?.errors ?? 0, requests()?.count ?? 0)}
              unit="ratio"
              tone={(requests()?.errors ?? 0) > 0 ? "error" : "default"}
              detail={`${(requests()?.errors ?? 0).toLocaleString()} failed`}
              trend={errorRateChart()[0]?.values}
              trendColor="error"
            />
            <For each={PERCENTILES}>
              {(p, i) => (
                <Stat
                  label={`${p.toUpperCase()} latency`}
                  value={requests()?.latency?.[p]}
                  unit="duration"
                  trend={latencyChart()[i()]?.values}
                  trendColor={p}
                />
              )}
            </For>
          </div>

          <Show when={frame()}>
            {(shown) => (
              <div class="grid grid-cols-1 gap-4 xl:grid-cols-3">
                <ChartPanel
                  title="Requests"
                  description="By step"
                  kind="bar"
                  unit="count"
                  frame={shown()}
                  series={requestChart()}
                  loading={fetched.loading()}
                  onZoom={zoom}
                />
                <ChartPanel
                  title="Latency"
                  description="Percentiles of the durations"
                  kind="line"
                  unit="duration"
                  frame={shown()}
                  series={latencyChart()}
                  loading={fetched.loading()}
                  onZoom={zoom}
                  empty="No requests in this range"
                />
                <ChartPanel
                  title="Error rate"
                  description="The share that failed"
                  kind="area"
                  unit="ratio"
                  frame={shown()}
                  series={errorRateChart()}
                  loading={fetched.loading()}
                  onZoom={zoom}
                  empty="No requests in this range"
                />
              </div>
            )}
          </Show>

          <Panel
            title="Spans"
            description={`The newest ${SPANS} spans of the operation; a span opens its trace`}
            flush
          >
            <Show when={spans.data()}>
              {(body) => (
                <Show
                  when={body().spans.length > 0}
                  fallback={<Empty>The operation has no spans in this range.</Empty>}
                >
                  <SpanList
                    spans={body().spans}
                    selected={undefined}
                    onSelect={(span) => {
                      if (span)
                        navigate(`/traces/${span.trace_id}${search({ span: span.span_id })}`);
                    }}
                  />
                </Show>
              )}
            </Show>
          </Panel>
        </div>
      </div>
    </Modal>
  );
}
