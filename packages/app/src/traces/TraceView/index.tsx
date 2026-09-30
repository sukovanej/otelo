import { type Accessor, createMemo, createSignal, type JSX, Match, Show, Switch } from "solid-js";

import { getTrace, type LogLine, toQueryString } from "@otelo/api";
import { Button, Callout, EmptyMessage, Tabs } from "@otelo/ui";
import { Panel, Value } from "@otelo/viz";

import { pageContent } from "../../classes";
import { formatCount } from "../../count";
import { createFetch } from "../../fetch";
import LinePanel from "../../logs/LinePanel";
import LogLines, { toLineKey } from "../../logs/LogLines";
import PageBar from "../../PageBar";
import ServiceName from "../../ServiceName";
import { formatDateTime, formatTime, parseTime } from "../../time";
import { isFailedSpan } from "../span";
import SpanPanel from "../span-panel";
import SpanTitle from "../SpanTitle";
import { buildSpanTree, measureTraceNanos } from "../tree";
import TraceViewWaterfall from "./trace-view-waterfall";

export type TraceTab = "spans" | "logs";

interface TraceState {
  readonly tab: Accessor<TraceTab>;
  readonly setTab: (tab: TraceTab) => void;
  readonly spanId: Accessor<string | undefined>;
  readonly setSpanId: (spanId: string | undefined) => void;
  readonly line: Accessor<LogLine | undefined>;
  readonly setLine: (line: LogLine | undefined) => void;
}

interface TraceViewProps {
  readonly id: string;
  readonly state: TraceState;
  readonly onFilterSpans: (term: string) => void;
  readonly onFilterLogs: (term: string) => void;
  readonly lead?: JSX.Element;
  readonly actions?: JSX.Element;
}

export default function TraceView(props: TraceViewProps) {
  const state = () => props.state;
  const fetched = createFetch(
    () => props.id,
    (id, signal) => getTrace(id, {}, signal),
  );
  // The answer of an earlier trace does not show while this one loads.
  const trace = () => {
    const answer = fetched.data();
    return answer?.trace_id === props.id.toLowerCase() ? answer : undefined;
  };
  const rows = createMemo(() => buildSpanTree(trace()?.spans ?? []));
  const rootSpan = () => rows()[0]?.span;
  const failedSpanCount = () => trace()?.spans.filter(isFailedSpan).length ?? 0;

  const selectedSpan = () => trace()?.spans.find((span) => span.span_id === state().spanId());
  const selectedLineKey = () => {
    const line = state().line();
    return line && toLineKey(line);
  };

  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <PageBar
        fetched={fetched}
        top={
          <div class="flex items-center gap-3">
            {props.lead}
            <h1 class="min-w-0 font-mono text-md font-semibold">
              <Show when={rootSpan()} fallback="Trace">
                {(span) => (
                  <SpanTitle
                    variant="span"
                    name={span().name}
                    attributes={span().attributes}
                    error={failedSpanCount() > 0}
                  />
                )}
              </Show>
            </h1>
            <Show when={rootSpan()}>
              {(span) => (
                <span class="min-w-0 font-mono text-muted">
                  <ServiceName name={span().service} resource={span().resource} />
                </span>
              )}
            </Show>
            <span class="flex-1" />
            <span class="font-mono text-xs text-muted" title="Trace ID">
              {props.id}
            </span>
            {props.actions}
          </div>
        }
        end={
          <Button disabled={fetched.loading()} onClick={() => fetched.reload()}>
            Reload
          </Button>
        }
      >
        <Tabs
          label="View"
          options={[
            { value: "spans", label: `Spans${trace() ? ` (${trace()?.spans.length})` : ""}` },
            { value: "logs", label: `Logs${trace() ? ` (${trace()?.logs.length})` : ""}` },
          ]}
          value={state().tab()}
          onChange={(tab) => state().setTab(tab)}
        />
        <Show when={rootSpan()}>
          {(span) => (
            <span>
              <Value value={measureTraceNanos(rows())} unit="duration" /> from{" "}
              <time
                datetime={span().started_at}
                title={formatDateTime(parseTime(span().started_at))}
              >
                {formatTime(parseTime(span().started_at))}
              </time>
              <Show when={failedSpanCount() > 0}>
                , {formatCount(failedSpanCount(), "failed span")}
              </Show>
            </span>
          )}
        </Show>
      </PageBar>

      <div class="flex min-h-0 flex-1">
        <div class={`min-w-0 flex-1 bg-page ${pageContent}`}>
          <Show when={fetched.errorMessage()}>
            {(errorMessage) => (
              <div class="mb-3">
                <Callout tone="error">{errorMessage()}</Callout>
              </div>
            )}
          </Show>
          <Show when={trace()?.truncated}>
            <div class="mb-3">
              <Callout tone="hint">The trace has more spans or logs than the page shows.</Callout>
            </div>
          </Show>

          <Show when={trace()}>
            {(shownTrace) => (
              <Panel flush>
                <Switch>
                  <Match when={state().tab() === "spans"}>
                    <Show
                      when={rows().length > 0}
                      fallback={
                        <EmptyMessage>
                          The retention has logs of this trace, but no spans.
                        </EmptyMessage>
                      }
                    >
                      <TraceViewWaterfall
                        rows={rows()}
                        selectedSpanId={state().spanId()}
                        onSelect={(span) => state().setSpanId(span?.span_id)}
                      />
                    </Show>
                  </Match>
                  <Match when={state().tab() === "logs"}>
                    <Show
                      when={shownTrace().logs.length > 0}
                      fallback={
                        <EmptyMessage>No log line carries the ID of this trace.</EmptyMessage>
                      }
                    >
                      <LogLines
                        lines={shownTrace().logs}
                        selectedKey={selectedLineKey()}
                        onSelect={(line) => state().setLine(line)}
                      />
                    </Show>
                  </Match>
                </Switch>
              </Panel>
            )}
          </Show>
        </div>

        <Switch>
          <Match when={state().tab() === "spans" && selectedSpan()}>
            {(span) => (
              <SpanPanel
                span={span()}
                onOpenTrace={undefined}
                onFilter={props.onFilterSpans}
                onClose={() => state().setSpanId(undefined)}
              />
            )}
          </Match>
          <Match when={state().tab() === "logs" && state().line()}>
            {(line) => (
              <LinePanel
                line={line()}
                linksToTrace={false}
                onFilter={props.onFilterLogs}
                onClose={() => state().setLine(undefined)}
              />
            )}
          </Match>
        </Switch>
      </div>
    </div>
  );
}

export function createTraceState(
  tab: Accessor<TraceTab>,
  setTab: (tab: TraceTab) => void,
  spanId: Accessor<string | undefined>,
  setSpanId: (spanId: string | undefined) => void,
): TraceState {
  const [line, setLine] = createSignal<LogLine>();
  return { tab, setTab, spanId, setSpanId, line, setLine };
}

export function closePanel(state: TraceState): boolean {
  if (state.tab() === "logs" && state.line()) {
    state.setLine(undefined);
    return true;
  }
  if (state.tab() === "spans" && state.spanId()) {
    state.setSpanId(undefined);
    return true;
  }
  return false;
}

export function toTracePath(id: string, state: TraceState): string {
  return `/traces/${id}${toQueryString({
    view: state.tab() === "logs" ? "logs" : undefined,
    span: state.spanId(),
  })}`;
}
