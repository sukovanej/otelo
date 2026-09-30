import {
  A,
  type Params,
  type SearchParams,
  useNavigate,
  useParams,
  useSearchParams,
} from "@solidjs/router";
import { createMemo, Show } from "solid-js";

import { getCalls, getLogGroups, getService, getTraces, type TargetKey } from "@otelo/api";
import { Callout, EmptyMessage } from "@otelo/ui";
import { Panel } from "@otelo/viz";

import { link, pageContent } from "../../classes";
import LogGroupList from "../../logs/LogGroupList";
import { addTerm, quoteString } from "../../query";
import ServiceName from "../../ServiceName";
import TraceList from "../../traces/TraceList";
import { createRangeFetch, useRange } from "../range";
import RangeBar from "../range-bar";
import { isSameTarget, parseTarget } from "../target";
import ServicePageCalls, { type CallRow } from "./service-page-calls";
import ServicePageOperationModal from "./service-page-operation-modal";
import ServicePageOverview from "./service-page-overview";

const CLOSED_MODAL_PARAMS = {
  op: undefined,
  kind: undefined,
  call: undefined,
  system: undefined,
  target: undefined,
};

interface ServicePageParams extends Params {
  readonly name: string;
}

interface OpenModalSearchParams extends SearchParams {
  readonly op?: string;
  readonly kind?: string;
  readonly call?: string;
  readonly system?: string;
  readonly target?: string;
}

type OpenModal = OpenOperation | OpenCall;

interface OpenOperation {
  readonly variant: "operation";
  readonly name: string;
  readonly kind: number;
}

interface OpenCall {
  readonly variant: "call";
  readonly summary: string;
  readonly kind: number;
  readonly target: TargetKey;
}

export default function ServicePage() {
  const params = useParams<ServicePageParams>();
  const range = useRange();
  const navigate = useNavigate();
  // In the URL, so Back closes the modal and a link opens it.
  const [modalParams, setModalParams] = useSearchParams<OpenModalSearchParams>();
  const openModal = createMemo(
    (): OpenModal | undefined => {
      const kind = Number(modalParams.kind);
      if (modalParams.op === undefined || !Number.isInteger(kind)) return undefined;
      const target = parseTarget(modalParams.call, modalParams.system, modalParams.target);
      return target
        ? { variant: "call", summary: modalParams.op, kind, target }
        : { variant: "operation", name: modalParams.op, kind };
    },
    undefined,
    { equals: isSameOpenModal },
  );
  const name = () => params.name;
  const serviceTerm = () => `service = ${quoteString(name())}`;

  const fetchedService = createRangeFetch(
    range,
    () => ({ name: name() }),
    ({ name: serviceName, ...bounds }, signal) => getService(serviceName, bounds, signal),
  );
  const fetchedCalls = createRangeFetch(
    range,
    () => ({ name: name() }),
    ({ name: serviceName, ...bounds }, signal) => getCalls(serviceName, bounds, signal),
  );
  const fetchedErrorTraces = createRangeFetch(
    range,
    () => ({ q: `${serviceTerm()} error = true`, limit: 10 }),
    getTraces,
  );
  const fetchedErrorLogs = createRangeFetch(
    range,
    () => ({ q: `${serviceTerm()} level >= error`, limit: 10 }),
    getLogGroups,
  );

  const service = () => {
    const answer = fetchedService.data();
    return answer?.service === name() ? answer : undefined;
  };
  const linkToTraces = (query: string) => `/traces${range.toSearch({ q: query })}`;
  const linkToLogs = (query: string, view?: string) => `/logs${range.toSearch({ q: query, view })}`;
  const zoomRangeTo = (start: number, end: number) =>
    range.setRange(new Date(start).toISOString(), new Date(end).toISOString());

  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <RangeBar
        range={range}
        fetched={fetchedService}
        title={
          <div class="flex min-w-0 items-center gap-3">
            <A href={`/services${range.toSearch()}`} class={link}>
              Services
            </A>
            <span class="text-muted">/</span>
            <h1 class="m-0 min-w-0 font-mono text-md font-semibold">
              <ServiceName name={name()} resource={service()?.resource ?? {}} />
            </h1>
          </div>
        }
      >
        <span class="ml-3 flex gap-3">
          <A href={linkToTraces(serviceTerm())} class={link}>
            Traces
          </A>
          <A href={linkToLogs(serviceTerm())} class={link}>
            Logs
          </A>
        </span>
      </RangeBar>

      <div class={`min-h-0 flex-1 ${pageContent}`}>
        <Show when={fetchedService.errorMessage()}>
          {(errorMessage) => (
            <div class="mb-3">
              <Callout tone="error">{errorMessage()}</Callout>
            </div>
          )}
        </Show>
        <Show when={service()}>
          {(shownService) => (
            <Show
              when={shownService().stats.requests.count > 0 || shownService().stats.logs > 0}
              fallback={<EmptyMessage>{name()} sent no spans or logs in this range.</EmptyMessage>}
            >
              <div class="flex flex-col gap-4">
                <ServicePageOverview
                  service={shownService()}
                  loading={fetchedService.loading()}
                  onZoom={zoomRangeTo}
                  operationHref={(operation) =>
                    `/services/${encodeURIComponent(name())}${range.toSearch({
                      op: operation.name,
                      kind: String(operation.kind),
                    })}`
                  }
                  onOpenOperation={(operation) =>
                    setModalParams({
                      ...CLOSED_MODAL_PARAMS,
                      op: operation.name,
                      kind: String(operation.kind),
                    })
                  }
                />

                <Show when={fetchedCalls.data()}>
                  {(calls) => (
                    <Show when={calls().service === name() && calls().calls.count > 0}>
                      <ServicePageCalls
                        calls={calls()}
                        loading={fetchedCalls.loading()}
                        onZoom={zoomRangeTo}
                        callHref={(row) =>
                          `/services/${encodeURIComponent(name())}${range.toSearch(toCallModalParams(row))}`
                        }
                        onOpenCall={(row) => setModalParams(toCallModalParams(row))}
                      />
                    </Show>
                  )}
                </Show>

                <Panel
                  title="Failed traces"
                  description="The newest traces with a failed span of the service"
                  flush
                  actions={
                    <A href={linkToTraces(`${serviceTerm()} error = true`)} class={link}>
                      All failed traces
                    </A>
                  }
                >
                  <Show when={fetchedErrorTraces.data()}>
                    {(traces) => (
                      <Show
                        when={traces().traces.length > 0}
                        fallback={
                          <EmptyMessage>No trace of {name()} failed in this range.</EmptyMessage>
                        }
                      >
                        <TraceList
                          traces={traces().traces}
                          onOpen={(id) => navigate(`/traces/${id}`)}
                        />
                      </Show>
                    )}
                  </Show>
                </Panel>

                <Panel
                  title="Error logs"
                  description="The error logs of the service by message template"
                  flush
                  actions={
                    <A href={linkToLogs(`${serviceTerm()} level >= error`, "groups")} class={link}>
                      All error logs
                    </A>
                  }
                >
                  <Show when={fetchedErrorLogs.data()}>
                    {(logGroups) => (
                      <Show
                        when={logGroups().groups.length > 0}
                        fallback={
                          <EmptyMessage>{name()} logged no errors in this range.</EmptyMessage>
                        }
                      >
                        <LogGroupList
                          groups={logGroups().groups}
                          onShowLines={(term) =>
                            navigate(linkToLogs(addTerm(`${serviceTerm()} level >= error`, term)))
                          }
                        />
                      </Show>
                    )}
                  </Show>
                </Panel>
              </div>
            </Show>
          )}
        </Show>
      </div>

      <Show when={openModal()} keyed>
        {(modal) => (
          <ServicePageOperationModal
            {...modal}
            service={name()}
            range={range}
            onClose={() => setModalParams(CLOSED_MODAL_PARAMS)}
          />
        )}
      </Show>
    </div>
  );
}

function toCallModalParams(row: CallRow) {
  return {
    op: row.operation.summary,
    kind: String(row.operation.kind),
    call: row.target.type,
    system: row.target.system ?? undefined,
    target: row.target.name ?? undefined,
  };
}

function isSameOpenModal(previous: OpenModal | undefined, next: OpenModal | undefined): boolean {
  if (previous === undefined || next === undefined) return previous === next;
  if (previous.kind !== next.kind) return false;
  if (previous.variant === "operation") {
    return next.variant === "operation" && previous.name === next.name;
  }
  return (
    next.variant === "call" &&
    previous.summary === next.summary &&
    isSameTarget(previous.target, next.target)
  );
}
