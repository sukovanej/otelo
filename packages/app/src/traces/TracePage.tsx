import { A, useNavigate, useParams, useSearchParams } from "@solidjs/router";

import { search } from "@otelo/api";

import { link } from "../classes";
import { usePageKeys } from "../list";
import TraceView, { closePanel, createTraceState } from "./TraceView";

/**
 * The page of one trace. The tab and the selected span live in the URL, so
 * a link opens the same span.
 */
export default function TracePage() {
  const params = useParams<{ id: string }>();
  const [query, setQuery] = useSearchParams<{ view?: string; span?: string }>();
  const navigate = useNavigate();
  const state = createTraceState(
    () => (query.view === "logs" ? "logs" : "spans"),
    (tab) => setQuery({ view: tab === "spans" ? undefined : tab }),
    () => query.span,
    (id) => setQuery({ span: id }, { replace: true }),
  );
  usePageKeys({ onEscape: () => closePanel(state) });

  return (
    <TraceView
      id={params.id}
      state={state}
      // A filter leaves the trace for the list it narrows.
      onFilterSpans={(term) => navigate(`/traces${search({ view: "spans", q: term })}`)}
      onFilterLogs={(term) => navigate(`/logs${search({ q: term })}`)}
      lead={
        <>
          <A href="/traces" class={link}>
            Traces
          </A>
          <span class="text-muted">/</span>
        </>
      }
    />
  );
}
