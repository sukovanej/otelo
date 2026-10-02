import { type SearchParams, useNavigate, useSearchParams } from "@solidjs/router";
import { createSignal, Match, Show, snapshot, Switch } from "solid-js";

import {
  getSpans,
  getTraces,
  toQueryString,
  type Spans,
  type Traces,
  type TraceSpan,
} from "@otelo/api";
import { EmptyMessage } from "@otelo/ui";

import { formatCount } from "../../count";
import { createListState, usePageKeys } from "../../list";
import ListContent from "../../ListContent";
import { addTerm } from "../../query";
import QueryBar from "../../QueryBar";
import SpanPanel from "../span-panel";
import SpanList, { toSpanKey } from "../SpanList";
import TraceList from "../TraceList";
import TracesPageTraceModal from "./traces-page-trace-modal";

const VIEWS = [
  { value: "traces", label: "Traces" },
  { value: "spans", label: "Spans" },
] as const;

type TracesView = "traces" | "spans";

type TracesResult = TraceListResult | SpanListResult;

interface TraceListResult {
  readonly view: "traces";
  readonly body: Traces;
}

interface SpanListResult {
  readonly view: "spans";
  readonly body: Spans;
}

interface OpenTraceSearchParams extends SearchParams {
  readonly trace?: string;
}

export default function TracesPage() {
  const list = createListState<TracesView, TracesResult>({
    name: "traces",
    views: ["traces", "spans"],
    firstLimits: { traces: 50, spans: 200 },
    fetch: async (key, signal) =>
      key.view === "spans"
        ? { view: "spans", body: await getSpans(key, signal) }
        : { view: "traces", body: await getTraces(key, signal) },
  });

  // The span stays open when a reload or another query no longer brings it.
  // A snapshot, since a reload merges the next answer into the rows by position.
  const [selectedSpan, setSelectedSpan] = createSignal<TraceSpan>();
  const selectSpan = (span: TraceSpan | undefined) => setSelectedSpan(span && snapshot(span));
  const selectedKey = () => {
    const span = selectedSpan();
    return span && toSpanKey(span);
  };

  const [params, setParams] = useSearchParams<OpenTraceSearchParams>();
  const [initialSpanId, setInitialSpanId] = createSignal<string>();
  const openTrace = (traceId: string, spanId?: string) => {
    setInitialSpanId(spanId);
    setParams({ trace: traceId });
  };
  const navigate = useNavigate();

  let queryInput: HTMLInputElement | undefined;
  usePageKeys({ queryInput: () => queryInput, onEscape: () => setSelectedSpan(undefined) });

  const traces = () => {
    const result = list.shownResult();
    return result?.view === "traces" ? result.body : undefined;
  };
  const spans = () => {
    const result = list.shownResult();
    return result?.view === "spans" ? result.body : undefined;
  };

  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <QueryBar
        list={list}
        signal="spans"
        placeholder='service = "api" duration > 500ms error = true'
        views={VIEWS}
        ref={(el) => (queryInput = el)}
      >
        <Switch>
          <Match when={traces()}>
            {(body) => (
              <>
                {formatCount(body().traces.length, "trace")}
                {body().truncated ? ", newest first; more match" : ""}
              </>
            )}
          </Match>
          <Match when={spans()}>
            {(body) => (
              <>
                {formatCount(body().spans.length, "span")}
                {body().truncated ? ", newest first; more match" : ""}
              </>
            )}
          </Match>
        </Switch>
      </QueryBar>

      <ListContent
        list={list}
        signal="spans"
        singularNoun="span"
        panel={
          <Show when={list.view() === "spans" && selectedSpan()}>
            {(span) => (
              <SpanPanel
                span={span()}
                onFilter={list.addTerm}
                onOpenTrace={(openedSpan) => openTrace(openedSpan.trace_id, openedSpan.span_id)}
                onClose={() => setSelectedSpan(undefined)}
              />
            )}
          </Show>
        }
      >
        <Switch>
          <Match when={traces()}>
            {(body) => (
              <Show
                when={body().traces.length > 0}
                fallback={
                  <EmptyMessage>
                    No trace in this range has a span that matches the query.
                  </EmptyMessage>
                }
              >
                <TraceList traces={body().traces} onOpen={openTrace} />
              </Show>
            )}
          </Match>
          <Match when={spans()}>
            {(body) => (
              <Show
                when={body().spans.length > 0}
                fallback={<EmptyMessage>No spans in this range match the query.</EmptyMessage>}
              >
                <SpanList spans={body().spans} selectedKey={selectedKey()} onSelect={selectSpan} />
              </Show>
            )}
          </Match>
        </Switch>
      </ListContent>

      <Show when={params.trace} keyed>
        {(id) => (
          <TracesPageTraceModal
            id={id}
            initialSpanId={initialSpanId()}
            // A span filter narrows the list under the modal, and shows its spans.
            onFilterSpans={(term) =>
              setParams({ trace: undefined, view: "spans", q: addTerm(list.query(), term) })
            }
            onFilterLogs={(term) => navigate(`/logs${toQueryString({ q: term })}`)}
            onClose={() => setParams({ trace: undefined })}
          />
        )}
      </Show>
    </div>
  );
}
