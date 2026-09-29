import { useNavigate, useSearchParams } from "@solidjs/router";
import { createSignal, Match, Show, Switch } from "solid-js";

import { getSpans, getTraces, search, type TraceSpan, type Spans, type Traces } from "@otelo/api";

import { count, createList, usePageKeys } from "../list";
import { Empty, ListContent, QueryBar } from "../ListFrame";
import { addTerm } from "../query";
import SpanList, { spanKey } from "./SpanList";
import SpanPanel from "./SpanPanel";
import TraceList from "./TraceList";
import TraceModal from "./TraceModal";

type View = "traces" | "spans";

const VIEWS = [
  { value: "traces", label: "Traces" },
  { value: "spans", label: "Spans" },
] as const;

type Result = { view: "traces"; body: Traces } | { view: "spans"; body: Spans };

/**
 * Traces that have a span the query keeps, or the spans themselves, for a
 * query and a range. A trace opens in a modal over the list. The query, the
 * range, the view, live mode, and the open trace live in the URL, so a link
 * opens the same page, and Back closes the trace.
 */
export default function TracesPage() {
  const list = createList<View, Result>({
    views: ["traces", "spans"],
    page: { traces: 50, spans: 200 },
    fetch: async (k, signal) =>
      k.view === "spans"
        ? { view: "spans", body: await getSpans(k, signal) }
        : { view: "traces", body: await getTraces(k, signal) },
  });

  // The span open in the panel. It stays open when a reload or another
  // query no longer brings it.
  const [selected, setSelected] = createSignal<TraceSpan>();
  const selectedKey = () => {
    const span = selected();
    return span && spanKey(span);
  };

  const [params, setParams] = useSearchParams<{ trace?: string }>();
  // The span a trace opens on, when the panel of a span opened it.
  const [openSpan, setOpenSpan] = createSignal<string>();
  const openTrace = (id: string, span?: string) => {
    setOpenSpan(span);
    setParams({ trace: id });
  };
  const navigate = useNavigate();

  let queryInput: HTMLInputElement | undefined;
  usePageKeys({ query: () => queryInput, onEscape: () => setSelected(undefined) });

  const traces = () => {
    const result = list.current();
    return result?.view === "traces" ? result.body : undefined;
  };
  const spans = () => {
    const result = list.current();
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
                {count(body().traces.length, "trace")}
                {body().truncated ? ", newest first; more match" : ""}
              </>
            )}
          </Match>
          <Match when={spans()}>
            {(body) => (
              <>
                {count(body().spans.length, "span")}
                {body().truncated ? ", newest first; more match" : ""}
              </>
            )}
          </Match>
        </Switch>
      </QueryBar>

      <ListContent
        list={list}
        signal="spans"
        noun="span"
        panel={
          <Show when={list.view() === "spans" && selected()}>
            {(span) => (
              <SpanPanel
                span={span()}
                onFilter={list.filter}
                onOpenTrace={(shown) => openTrace(shown.trace_id, shown.span_id)}
                onClose={() => setSelected(undefined)}
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
                fallback={<Empty>No trace in this range has a span that matches the query.</Empty>}
              >
                <TraceList traces={body().traces} onOpen={openTrace} />
              </Show>
            )}
          </Match>
          <Match when={spans()}>
            {(body) => (
              <Show
                when={body().spans.length > 0}
                fallback={<Empty>No spans in this range match the query.</Empty>}
              >
                <SpanList spans={body().spans} selected={selectedKey()} onSelect={setSelected} />
              </Show>
            )}
          </Match>
        </Switch>
      </ListContent>

      <Show when={params.trace} keyed>
        {(id) => (
          <TraceModal
            id={id}
            span={openSpan()}
            // A span filter narrows the list under the modal, and shows its spans.
            onFilterSpans={(term) =>
              setParams({ trace: undefined, view: "spans", q: addTerm(list.q(), term) })
            }
            onFilterLogs={(term) => navigate(`/logs${search({ q: term })}`)}
            onClose={() => setParams({ trace: undefined })}
          />
        )}
      </Show>
    </div>
  );
}
