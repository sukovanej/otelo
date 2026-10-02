import { useNavigate } from "@solidjs/router";
import { createMemo, For, Show } from "solid-js";

import {
  getCall,
  getOperation,
  getSpans,
  type OperationDetail,
  toQueryString,
  type TargetKey,
  type TraceSpan,
} from "@otelo/api";
import { Callout, CloseButton, EmptyMessage, Modal, SpanKindBadge } from "@otelo/ui";
import { ChartPanel, formatValue, Panel, Stat } from "@otelo/viz";

import { link, pageContent } from "../../classes";
import { quoteString } from "../../query";
import { entersService, toKindName } from "../../traces/span";
import SpanList from "../../traces/SpanList";
import SpanTitle from "../../traces/SpanTitle";
import { toTimeFrame } from "../frame";
import { createRangeFetch, type RangeState } from "../range";
import { PERCENTILES, toErrorRateSeries, toLatencySeries, toRequestSeries } from "../series";
import { measureSeconds, toRate, toShare } from "../stats";
import { applySummaryToAttributes, toTargetParams } from "../target";
import ServicePageTargetName from "./service-page-target-name";

const LISTED_SPAN_LIMIT = 50;

interface OperationResult {
  readonly detail: OperationDetail;
  readonly spans: ReadonlyArray<TraceSpan>;
  readonly spanQuery: string;
}

type ServicePageOperationModalProps = OperationModalProps | CallModalProps;

interface ModalBaseProps {
  readonly service: string;
  readonly kind: number;
  readonly range: RangeState;
  readonly onClose: () => void;
}

interface OperationModalProps extends ModalBaseProps {
  readonly variant: "operation";
  readonly name: string;
}

interface CallModalProps extends ModalBaseProps {
  readonly variant: "call";
  readonly summary: string;
  readonly target: TargetKey;
}

export default function ServicePageOperationModal(props: ServicePageOperationModalProps) {
  const navigate = useNavigate();
  const spanName = () => (props.variant === "call" ? props.summary : props.name);
  const callTarget = () => (props.variant === "call" ? props.target : undefined);
  const countLabel = () => (props.variant === "call" ? "Calls" : "Requests");
  const subjectNoun = () => (props.variant === "call" ? "call" : "operation");
  const fetched = createRangeFetch(
    props.range,
    "operation",
    () => ({ service: props.service, name: spanName(), kind: props.kind, target: callTarget() }),
    async ({ service, name, kind, target, since, until }, signal): Promise<OperationResult> => {
      if (target) {
        const call = await getCall(
          service,
          { summary: name, kind, since, until, ...toTargetParams(target) },
          signal,
        );
        const kindTerm = kind === 0 ? "" : ` kind = ${toKindName(kind)}`;
        const spanQuery = `service = ${quoteString(service)}${kindTerm} ${call.query}`.trim();
        return { detail: call, spans: call.spans, spanQuery };
      }
      const spanQuery = writeOperationQuery(service, name, kind);
      const [detail, list] = await Promise.all([
        getOperation(service, { operation: name, kind, since, until }, signal),
        getSpans({ q: spanQuery, limit: LISTED_SPAN_LIMIT, since, until }, signal),
      ]);
      return { detail, spans: list.spans, spanQuery };
    },
  );

  const detail = () => fetched.data()?.detail;
  const titleAttributes = () => {
    const attributes = detail()?.attributes ?? {};
    return props.variant === "call"
      ? applySummaryToAttributes(props.target.type, props.summary, attributes)
      : attributes;
  };
  const timeFrame = createMemo(() => {
    const operation = detail();
    return operation && toTimeFrame(operation);
  });
  const requestSeries = createMemo(() => toRequestSeries(detail()?.buckets ?? []));
  const latencySeries = createMemo(() => toLatencySeries(detail()?.buckets ?? []));
  const errorRateSeries = createMemo(() => toErrorRateSeries(detail()?.buckets ?? []));
  const requests = () => detail()?.requests;
  const zoomRangeTo = (start: number, end: number) =>
    props.range.setRange(new Date(start).toISOString(), new Date(end).toISOString());

  return (
    <Modal
      label={`${props.variant === "call" ? "Call" : "Operation"} ${spanName()}`}
      onClose={props.onClose}
    >
      <div class="relative z-20 flex shrink-0 items-center gap-3 bg-surface px-4 py-2.5 shadow-(--raised)">
        <h1 class="m-0 min-w-0 max-w-[60ch] font-mono text-md font-semibold">
          <SpanTitle variant="operation" name={spanName()} attributes={titleAttributes()} />
        </h1>
        <SpanKindBadge kind={toKindName(props.kind)} />
        <Show when={callTarget()}>
          {(target) => (
            <span class="flex max-w-64 shrink-0 items-baseline gap-1.5 text-muted">
              to <ServicePageTargetName target={target()} />
            </span>
          )}
        </Show>
        <span class="flex-1" />
        <Show when={fetched.loading()}>
          <span class="shrink-0 whitespace-nowrap text-muted" aria-live="polite">
            Loading…
          </span>
        </Show>
        <a
          href={`/traces${toQueryString({ view: "spans", q: fetched.data()?.spanQuery, since: props.range.since(), until: props.range.until() || undefined })}`}
          class={`shrink-0 whitespace-nowrap ${link}`}
          title={`Every span of the ${subjectNoun()} on the traces page`}
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
        <div class="flex flex-col gap-4">
          <div class="grid grid-cols-[repeat(auto-fit,minmax(170px,1fr))] gap-3">
            <Stat
              label={countLabel()}
              value={requests()?.count}
              unit="count"
              detail={formatValue(
                toRate(
                  requests()?.count ?? 0,
                  measureSeconds(detail()?.start_at ?? "", detail()?.end_at ?? ""),
                ),
                "rate",
              )}
              trend={detail()?.buckets.map((bucket) => bucket.requests.count)}
            />
            <Stat
              label="Error rate"
              value={toShare(requests()?.errors ?? 0, requests()?.count ?? 0)}
              unit="ratio"
              tone={(requests()?.errors ?? 0) > 0 ? "error" : undefined}
              detail={`${(requests()?.errors ?? 0).toLocaleString()} failed`}
              trend={errorRateSeries()[0]?.values}
              trendColor="error"
            />
            <For each={PERCENTILES}>
              {(percentile, index) => (
                <Stat
                  label={`${percentile.toUpperCase()} latency`}
                  value={requests()?.latency?.[percentile]}
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
                  title={countLabel()}
                  description="By step"
                  kind="bar"
                  unit="count"
                  frame={frame()}
                  series={requestSeries()}
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
                  emptyMessage={`No ${countLabel().toLowerCase()} in this range`}
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
                  emptyMessage={`No ${countLabel().toLowerCase()} in this range`}
                />
              </div>
            )}
          </Show>

          <Panel
            title="Spans"
            description={`The newest ${LISTED_SPAN_LIMIT} spans of the ${subjectNoun()}; a span opens its trace`}
            flush
          >
            <Show when={fetched.data()}>
              {(result) => (
                <Show
                  when={result().spans.length > 0}
                  fallback={
                    <EmptyMessage>The {subjectNoun()} has no spans in this range.</EmptyMessage>
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
      </div>
    </Modal>
  );
}

function writeOperationQuery(service: string, name: string, kind: number): string {
  const kindTerm = kind === 0 ? "" : ` kind = ${toKindName(kind)}`;
  const rootTerm = entersService(kind) ? "" : " root = true";
  return `service = ${quoteString(service)} name = ${quoteString(name)}${kindTerm}${rootTerm}`;
}
