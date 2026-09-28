import { Button, Callout, Tabs } from "@siner/ui";
import { type Accessor, createMemo, createSignal, type JSX, Match, Show, Switch } from "solid-js";
import { getTrace, type LogLine, search } from "@siner/api";
import Duration from "../Duration";
import { createFetch } from "../fetch";
import { count } from "../list";
import { Empty } from "../ListFrame";
import LinePanel from "../logs/LinePanel";
import LogLines, { lineKey } from "../logs/LogLines";
import Service from "../Service";
import { formatDateTime, formatTime, parseTime } from "../time";
import SpanPanel from "./SpanPanel";
import SpanTitle from "./SpanTitle";
import { spanTree, traceLength } from "./tree";
import Waterfall from "./Waterfall";

export type TraceTab = "spans" | "logs";

/** What a trace view shows: its tab, and the span or the log line open in
 * its panel. The page keeps the tab and the span in the URL, and the modal
 * keeps them to itself. */
export interface TraceState {
  tab: Accessor<TraceTab>;
  setTab: (tab: TraceTab) => void;
  /** The ID of the selected span. */
  span: Accessor<string | undefined>;
  setSpan: (id: string | undefined) => void;
  line: Accessor<LogLine | undefined>;
  setLine: (line: LogLine | undefined) => void;
}

/** A state whose tab and span are the given signals, and whose line lives
 * here. */
export function createTraceState(
  tab: Accessor<TraceTab>,
  setTab: (tab: TraceTab) => void,
  span: Accessor<string | undefined>,
  setSpan: (id: string | undefined) => void,
): TraceState {
  const [line, setLine] = createSignal<LogLine>();
  return { tab, setTab, span, setSpan, line, setLine };
}

/** Closes the panel of the tab the state shows. False when none is open. */
export function closePanel(state: TraceState): boolean {
  if (state.tab() === "logs" && state.line()) {
    state.setLine(undefined);
    return true;
  }
  if (state.tab() === "spans" && state.span()) {
    state.setSpan(undefined);
    return true;
  }
  return false;
}

/** The address of the page of a trace, with the tab and the span that
 * `state` shows. */
export const tracePath = (id: string, state: TraceState) =>
  `/traces/${id}${search({
    view: state.tab() === "logs" ? "logs" : undefined,
    span: state.span(),
  })}`;

/**
 * One trace from the whole retention: its spans as a waterfall, or the logs
 * that carry its ID, with the selected one in a panel beside them. `lead`
 * goes before the name of the trace and `actions` at the end of its line.
 */
export default function TraceView(props: {
  id: string;
  state: TraceState;
  /** Adds a term to the query of the span list, or of the logs. */
  onFilterSpans: (term: string) => void;
  onFilterLogs: (term: string) => void;
  lead?: JSX.Element;
  actions?: JSX.Element;
}) {
  const state = () => props.state;
  const fetched = createFetch(
    () => props.id,
    (id, signal) => getTrace(id, {}, signal),
  );
  // The answer of an earlier trace does not show while this one loads.
  const trace = () => {
    const data = fetched.data();
    return data?.trace_id === props.id.toLowerCase() ? data : undefined;
  };
  const rows = createMemo(() => spanTree(trace()?.spans ?? []));
  const root = () => rows()[0]?.span;
  const errors = () => trace()?.spans.filter((span) => span.error).length ?? 0;

  const selectedSpan = () => trace()?.spans.find((span) => span.span_id === state().span());
  const selectedLineKey = () => {
    const line = state().line();
    return line && lineKey(line);
  };

  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <div class="relative z-20 shrink-0 bg-surface px-4 pt-3.5 shadow-(--raised)">
        <div class="flex items-center gap-3">
          {props.lead}
          <h1 class="min-w-0 font-mono text-md font-semibold">
            <Show when={root()} fallback="Trace">
              {(span) => (
                <SpanTitle name={span().name} attributes={span().attributes} error={errors() > 0} />
              )}
            </Show>
          </h1>
          <Show when={root()}>
            {(span) => (
              <span class="min-w-0 font-mono text-muted">
                <Service name={span().service} resource={span().resource} />
              </span>
            )}
          </Show>
          <span class="flex-1" />
          <span class="font-mono text-xs text-muted" title="Trace ID">
            {props.id}
          </span>
          {props.actions}
        </div>

        <div class="flex items-center gap-3 pt-3 pb-2.5 text-muted">
          <Tabs
            label="View"
            options={[
              { value: "spans", label: `Spans${trace() ? ` (${trace()?.spans.length})` : ""}` },
              { value: "logs", label: `Logs${trace() ? ` (${trace()?.logs.length})` : ""}` },
            ]}
            value={state().tab()}
            onChange={(tab) => state().setTab(tab)}
          />
          <Show when={root()}>
            {(span) => (
              <span>
                <Duration nanos={traceLength(rows())} /> from{" "}
                <time datetime={span().time} title={formatDateTime(parseTime(span().time))}>
                  {formatTime(parseTime(span().time))}
                </time>
                <Show when={errors() > 0}>, {count(errors(), "failed span")}</Show>
              </span>
            )}
          </Show>
          <span class="flex-1" />
          <Show when={fetched.loading()}>
            <span aria-live="polite">Loading…</span>
          </Show>
          <Show when={fetched.updated()}>
            {(updated) => <span>Updated {formatTime(updated())}</span>}
          </Show>
          <Button disabled={fetched.loading()} onClick={() => fetched.reload()}>
            Reload
          </Button>
        </div>
      </div>

      <div class="flex min-h-0 flex-1">
        <div class="min-w-0 flex-1 overflow-y-auto px-4 pb-8">
          <Show when={fetched.error()}>
            {(error) => (
              <div class="mt-2">
                <Callout tone="error">{error()}</Callout>
              </div>
            )}
          </Show>
          <Show when={trace()?.truncated}>
            <div class="mt-2">
              <Callout tone="hint">The trace has more spans or logs than the page shows.</Callout>
            </div>
          </Show>

          <Show when={trace()}>
            {(shown) => (
              <Switch>
                <Match when={state().tab() === "spans"}>
                  <Show
                    when={rows().length > 0}
                    fallback={<Empty>The retention has logs of this trace, but no spans.</Empty>}
                  >
                    <Waterfall
                      rows={rows()}
                      selected={state().span()}
                      onSelect={(span) => state().setSpan(span?.span_id)}
                    />
                  </Show>
                </Match>
                <Match when={state().tab() === "logs"}>
                  <Show
                    when={shown().logs.length > 0}
                    fallback={<Empty>No log line carries the ID of this trace.</Empty>}
                  >
                    <LogLines
                      lines={shown().logs}
                      selected={selectedLineKey()}
                      onSelect={(line) => state().setLine(line)}
                    />
                  </Show>
                </Match>
              </Switch>
            )}
          </Show>
        </div>

        <Switch>
          <Match when={state().tab() === "spans" && selectedSpan()}>
            {(span) => (
              <SpanPanel
                span={span()}
                inTrace
                onFilter={props.onFilterSpans}
                onClose={() => state().setSpan(undefined)}
              />
            )}
          </Match>
          <Match when={state().tab() === "logs" && state().line()}>
            {(line) => (
              <LinePanel
                line={line()}
                inTrace
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
