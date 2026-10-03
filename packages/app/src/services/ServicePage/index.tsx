import {
  type Params,
  type SearchParams,
  useNavigate,
  useParams,
  useSearchParams,
} from "@solidjs/router";
import { createMemo, Errored, Show } from "solid-js";

import { getLogGroups, getService, getSpanGroups, getTraces, type SpanGroup } from "@otelo/api";
import { Callout, EmptyMessage } from "@otelo/ui";
import { Panel } from "@otelo/viz";

import { link, pageContent } from "../../classes";
import FetchErrorBoundary from "../../FetchErrorBoundary";
import LogGroupList from "../../logs/LogGroupList";
import { decodePathSegment } from "../../path";
import { addTerm, quoteString } from "../../query";
import ServiceName from "../../ServiceName";
import TraceList from "../../traces/TraceList";
import { createRangeFetch, useRange } from "../range";
import RangeBar from "../range-bar";
import {
  isSpanGroupingKind,
  SPAN_GROUPINGS,
  type SpanGrouping,
  type SpanGroupingKind,
  writeGroupFilter,
} from "../span-grouping";
import ServicePageOverview from "./service-page-overview";
import ServicePageQueries from "./service-page-queries";
import ServicePageResources from "./service-page-resources";
import ServicePageSpanGroupModal from "./service-page-span-group-modal";
import ServicePageSpanGroups from "./service-page-span-groups";

const CLOSED_MODAL_PARAMS = { group: undefined, spans: undefined };

interface ServicePageParams extends Params {
  readonly name: string;
}

interface OpenModalSearchParams extends SearchParams {
  readonly group?: string;
  readonly spans?: string;
}

interface OpenModal {
  readonly kind: SpanGroupingKind;
  readonly filter: string;
}

export default function ServicePage() {
  const params = useParams<ServicePageParams>();
  const range = useRange();
  const navigate = useNavigate();
  // In the URL, so Back closes the modal and a link opens it.
  const [modalParams, setModalParams] = useSearchParams<OpenModalSearchParams>();
  const openModal = createMemo(
    (): OpenModal | undefined => {
      const { group, spans } = modalParams;
      if (!isSpanGroupingKind(group) || spans === undefined) return undefined;
      return { kind: group, filter: spans };
    },
    { equals: isSameOpenModal },
  );
  const name = () => decodePathSegment(params.name);
  const serviceTerm = () => `service = ${quoteString(name())}`;

  const fetchedService = createRangeFetch(
    range,
    "service",
    () => ({ name: name() }),
    ({ name: serviceName, ...bounds }, signal) => getService(serviceName, bounds, signal),
  );
  const createSpanGroupFetch = (grouping: SpanGrouping) =>
    createRangeFetch(
      range,
      `span-groups-${grouping.kind}`,
      () => ({ q: grouping.writeFilter(name()), by: grouping.by.join(",") }),
      getSpanGroups,
    );
  const fetchedRoutes = createSpanGroupFetch(SPAN_GROUPINGS.route);
  const fetchedQueries = createSpanGroupFetch(SPAN_GROUPINGS.query);
  const fetchedErrorTraces = createRangeFetch(
    range,
    "traces",
    () => ({ q: `${serviceTerm()} error = true`, limit: 10 }),
    getTraces,
  );
  const fetchedErrorLogs = createRangeFetch(
    range,
    "log-groups",
    () => ({ q: `${serviceTerm()} level >= error`, limit: 10 }),
    getLogGroups,
  );

  const service = () => {
    const answer = fetchedService.data();
    return answer?.service === name() ? answer : undefined;
  };
  const linkToTraces = (query: string) => `/traces${range.toSearch({ q: query })}`;
  const linkToLogs = (query: string, view?: string) => `/logs${range.toSearch({ q: query, view })}`;
  const toGroupModalParams = (grouping: SpanGrouping, group: SpanGroup) => ({
    group: grouping.kind,
    spans: writeGroupFilter(grouping.writeFilter(name()), grouping.by, group),
  });
  const groupHref = (grouping: SpanGrouping) => (group: SpanGroup) =>
    `/services/${encodeURIComponent(name())}${range.toSearch(toGroupModalParams(grouping, group))}`;
  const openGroup = (grouping: SpanGrouping) => (group: SpanGroup) =>
    setModalParams(toGroupModalParams(grouping, group));
  const zoomRangeTo = (start: number, end: number) =>
    range.setRange(new Date(start).toISOString(), new Date(end).toISOString());

  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <RangeBar
        range={range}
        fetched={fetchedService}
        title={
          <div class="flex min-w-0 items-center gap-3">
            <a href={`/services${range.toSearch()}`} class={link}>
              Services
            </a>
            <span class="text-muted">/</span>
            <h1 class="m-0 min-w-0 font-mono text-md font-semibold">
              <Errored fallback={<ServiceName name={name()} resource={{}} />}>
                <ServiceName name={name()} resource={service()?.resource ?? {}} />
              </Errored>
            </h1>
          </div>
        }
      >
        <span class="ml-3 flex gap-3">
          <a href={linkToTraces(serviceTerm())} class={link}>
            Traces
          </a>
          <a href={linkToLogs(serviceTerm())} class={link}>
            Logs
          </a>
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
        <FetchErrorBoundary>
          <Show when={service()}>
            {(shownService) => (
              <Show
                when={shownService().stats.spans > 0 || shownService().stats.logs > 0}
                fallback={
                  <EmptyMessage>{name()} sent no spans or logs in this range.</EmptyMessage>
                }
              >
                <div class="flex flex-col gap-4">
                  <ServicePageOverview
                    service={shownService()}
                    loading={fetchedService.loading()}
                    onZoom={zoomRangeTo}
                  />

                  <Show when={fetchedRoutes.errorMessage()}>
                    {(errorMessage) => <Callout tone="error">{errorMessage()}</Callout>}
                  </Show>
                  <FetchErrorBoundary>
                    <Show when={fetchedRoutes.data()}>
                      {(routes) => (
                        <Show when={routes().groups.length > 0}>
                          <ServicePageSpanGroups
                            title="Routes"
                            description="The HTTP requests by method and route"
                            nameLabel="Route"
                            countLabel="Requests"
                            groups={routes()}
                            loading={fetchedRoutes.loading()}
                            groupHref={groupHref(SPAN_GROUPINGS.route)}
                            onOpenGroup={openGroup(SPAN_GROUPINGS.route)}
                          />
                        </Show>
                      )}
                    </Show>
                  </FetchErrorBoundary>

                  <ServicePageResources service={name()} range={range} onZoom={zoomRangeTo} />

                  <Show when={fetchedQueries.errorMessage()}>
                    {(errorMessage) => <Callout tone="error">{errorMessage()}</Callout>}
                  </Show>
                  <FetchErrorBoundary>
                    <Show when={fetchedQueries.data()}>
                      {(queries) => (
                        <Show when={queries().spans.count > 0}>
                          <ServicePageQueries
                            queries={queries()}
                            loading={fetchedQueries.loading()}
                            onZoom={zoomRangeTo}
                            queryHref={groupHref(SPAN_GROUPINGS.query)}
                            onOpenQuery={openGroup(SPAN_GROUPINGS.query)}
                          />
                        </Show>
                      )}
                    </Show>
                  </FetchErrorBoundary>

                  <Panel
                    title="Failed traces"
                    description="The newest traces with a failed span of the service"
                    flush
                    actions={
                      <a href={linkToTraces(`${serviceTerm()} error = true`)} class={link}>
                        All failed traces
                      </a>
                    }
                  >
                    <Show when={fetchedErrorTraces.errorMessage()}>
                      {(errorMessage) => (
                        <div class="p-3">
                          <Callout tone="error">{errorMessage()}</Callout>
                        </div>
                      )}
                    </Show>
                    <FetchErrorBoundary>
                      <Show when={fetchedErrorTraces.data()}>
                        {(traces) => (
                          <Show
                            when={traces().traces.length > 0}
                            fallback={
                              <EmptyMessage>
                                No trace of {name()} failed in this range.
                              </EmptyMessage>
                            }
                          >
                            <TraceList
                              traces={traces().traces}
                              onOpen={(id) => navigate(`/traces/${id}`)}
                            />
                          </Show>
                        )}
                      </Show>
                    </FetchErrorBoundary>
                  </Panel>

                  <Panel
                    title="Error logs"
                    description="The error logs of the service by message template"
                    flush
                    actions={
                      <a
                        href={linkToLogs(`${serviceTerm()} level >= error`, "groups")}
                        class={link}
                      >
                        All error logs
                      </a>
                    }
                  >
                    <Show when={fetchedErrorLogs.errorMessage()}>
                      {(errorMessage) => (
                        <div class="p-3">
                          <Callout tone="error">{errorMessage()}</Callout>
                        </div>
                      )}
                    </Show>
                    <FetchErrorBoundary>
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
                                navigate(
                                  linkToLogs(addTerm(`${serviceTerm()} level >= error`, term)),
                                )
                              }
                            />
                          </Show>
                        )}
                      </Show>
                    </FetchErrorBoundary>
                  </Panel>
                </div>
              </Show>
            )}
          </Show>
        </FetchErrorBoundary>
      </div>

      <Show when={openModal()} keyed>
        {(modal) => (
          <ServicePageSpanGroupModal
            kind={modal.kind}
            filter={modal.filter}
            range={range}
            onClose={() => setModalParams(CLOSED_MODAL_PARAMS)}
          />
        )}
      </Show>
    </div>
  );
}

function isSameOpenModal(previous: OpenModal | undefined, next: OpenModal | undefined): boolean {
  if (previous === undefined || next === undefined) return previous === next;
  return previous.kind === next.kind && previous.filter === next.filter;
}
