import {
  type Params,
  type SearchParams,
  useNavigate,
  useParams,
  useSearchParams,
} from "@solidjs/router";
import { Show } from "solid-js";

import { toQueryString } from "@otelo/api";

import { link } from "../classes";
import { usePageKeys } from "../list";
import TraceView, { closePanel, createTraceState } from "./TraceView";

interface TracePageParams extends Params {
  readonly id: string;
}

interface TracePageSearchParams extends SearchParams {
  readonly view?: string;
  readonly span?: string;
}

export default function TracePage() {
  const params = useParams<TracePageParams>();
  const [searchParams, setSearchParams] = useSearchParams<TracePageSearchParams>();
  const navigate = useNavigate();
  const state = createTraceState(
    () => (searchParams.view === "logs" ? "logs" : "spans"),
    (tab) => setSearchParams({ view: tab === "spans" ? undefined : tab }),
    () => searchParams.span,
    (spanId) => setSearchParams({ span: spanId }, { replace: true }),
  );
  usePageKeys({ onEscape: () => closePanel(state) });

  return (
    <Show when={params.id} keyed>
      {(id) => (
        <TraceView
          id={id}
          state={state}
          // A filter leaves the trace for the list it narrows.
          onFilterSpans={(term) => navigate(`/traces${toQueryString({ view: "spans", q: term })}`)}
          onFilterLogs={(term) => navigate(`/logs${toQueryString({ q: term })}`)}
          lead={
            <>
              <a href="/traces" class={link}>
                Traces
              </a>
              <span class="text-muted">/</span>
            </>
          }
        />
      )}
    </Show>
  );
}
