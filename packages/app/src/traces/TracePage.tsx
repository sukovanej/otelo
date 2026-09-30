import {
  A,
  type Params,
  type SearchParams,
  useNavigate,
  useParams,
  useSearchParams,
} from "@solidjs/router";

import { search } from "@otelo/api";

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
