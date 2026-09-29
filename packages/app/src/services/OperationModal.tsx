import { A, useNavigate } from "@solidjs/router";
import { createMemo, For, Show } from "solid-js";

import {
  getCall,
  getOperation,
  getSpans,
  type OperationDetail,
  search,
  type TargetKey,
  type TraceSpan,
} from "@siner/api";
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
import { summaryAttributes, targetParams } from "./target";
import TargetName from "./TargetName";

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

/** An operation or a call over a range, its newest spans, and the span
 * query of the traces page that shows them all. */
interface Shown {
  detail: OperationDetail;
  spans: TraceSpan[];
  query: string;
}

/**
 * One operation of a service over the page it was opened from, or with a
 * `target`, the calls of the service to it that do `name`, the summary of
 * the calls API: its numbers over the range, its requests or calls and
 * latency over time, and its newest spans, which open their traces. It
 * follows the range of the page, and dragging across a chart zooms that
 * range.
 */
export default function OperationModal(props: {
  service: string;
  name: string;
  kind: number;
  target?: TargetKey | undefined;
  range: Range;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const noun = () => (props.target ? "Calls" : "Requests");
  const what = () => (props.target ? "call" : "operation");
  const fetched = createRangeFetch(
    props.range,
    () => ({ service: props.service, name: props.name, kind: props.kind, target: props.target }),
    async ({ service, name, kind, target, since, until }, signal): Promise<Shown> => {
      if (target) {
        const call = await getCall(
          service,
          { summary: name, kind, since, until, ...targetParams(target) },
          signal,
        );
        const kindTerm = kind === 0 ? "" : ` kind = ${kindName(kind)}`;
        const query = `service = ${quote(service)}${kindTerm} ${call.query}`.trim();
        return { detail: call, spans: call.spans, query };
      }
      const query = operationQuery(service, name, kind);
      const [detail, list] = await Promise.all([
        getOperation(service, { operation: name, kind, since, until }, signal),
        getSpans({ q: query, limit: SPANS, since, until }, signal),
      ]);
      return { detail, spans: list.spans, query };
    },
  );

  const data = () => fetched.data()?.detail;
  const titleAttributes = () => {
    const attributes = data()?.attributes ?? {};
    return props.target ? summaryAttributes(props.target.type, props.name, attributes) : attributes;
  };
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
    <Modal label={`${props.target ? "Call" : "Operation"} ${props.name}`} onClose={props.onClose}>
      <div class="relative z-20 flex shrink-0 items-center gap-3 bg-surface px-4 py-2.5 shadow-(--raised)">
        <h1 class="m-0 min-w-0 font-mono text-md font-semibold">
          <SpanTitle
            name={props.name}
            attributes={titleAttributes()}
            error={false}
            status={false}
          />
        </h1>
        <KindBadge kind={props.kind} />
        <Show when={props.target}>
          {(target) => (
            <span class="flex max-w-64 shrink-0 items-baseline gap-1.5 text-muted">
              to <TargetName target={target()} />
            </span>
          )}
        </Show>
        <span class="flex-1" />
        <Show when={fetched.loading()}>
          <span class="shrink-0 whitespace-nowrap text-muted" aria-live="polite">
            Loading…
          </span>
        </Show>
        <A
          href={`/traces${search({ view: "spans", q: fetched.data()?.query, since: props.range.since(), until: props.range.until() || undefined })}`}
          class={`shrink-0 whitespace-nowrap ${link}`}
          title={`Every span of the ${what()} on the traces page`}
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
              label={noun()}
              value={requests()?.count}
              unit="count"
              detail={formatValue(
                rate(requests()?.count ?? 0, seconds(data()?.start_at ?? "", data()?.end_at ?? "")),
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
                  title={noun()}
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
                  empty={`No ${noun().toLowerCase()} in this range`}
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
                  empty={`No ${noun().toLowerCase()} in this range`}
                />
              </div>
            )}
          </Show>

          <Panel
            title="Spans"
            description={`The newest ${SPANS} spans of the ${what()}; a span opens its trace`}
            flush
          >
            <Show when={fetched.data()}>
              {(body) => (
                <Show
                  when={body().spans.length > 0}
                  fallback={<Empty>The {what()} has no spans in this range.</Empty>}
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
