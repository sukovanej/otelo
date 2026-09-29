import { A, useNavigate, useParams, useSearchParams } from "@solidjs/router";
import { createMemo, For, Show } from "solid-js";

import {
  getCalls,
  getLogGroups,
  getService,
  getTraces,
  type Operation,
  type Service as ServiceBody,
} from "@siner/api";
import { Callout } from "@siner/ui";
import {
  ChartPanel,
  type Column,
  formatValue,
  Panel,
  Stat,
  Table,
  type TimeFrame,
  type TimeSeries,
} from "@siner/viz";

import { link, pageContent } from "../classes";
import { Empty } from "../ListFrame";
import LogGroupList from "../logs/LogGroupList";
import { addTerm, quote } from "../query";
import Service from "../Service";
import KindBadge from "../traces/KindBadge";
import SpanTitle from "../traces/SpanTitle";
import TraceList from "../traces/TraceList";
import CallsSection, { type CallRow } from "./CallsSection";
import { timeFrame } from "./frame";
import OperationModal from "./OperationModal";
import { createRangeFetch, useRange } from "./range";
import { errorRateSeries, latencySeries, PERCENTILES, requestSeries } from "./series";
import { rate, seconds, share } from "./stats";
import { parseTarget, sameTarget } from "./target";
import Toolbar from "./Toolbar";

/** The search parameters of a closed modal. */
const closed = {
  op: undefined,
  kind: undefined,
  call: undefined,
  system: undefined,
  target: undefined,
};

/** The search parameters of the modal of a call. */
const callParams = (row: CallRow) => ({
  op: row.operation.summary,
  kind: String(row.operation.kind),
  call: row.target.type,
  system: row.target.system ?? undefined,
  target: row.target.name ?? undefined,
});

/**
 * One service in the range: its requests, error rate, latency, and logs as
 * numbers and over time, its requests by operation, the calls it makes by
 * target and by span name, and its newest failed traces and error logs.
 * Dragging across a chart zooms the range to that stretch. The range and
 * live mode live in the URL.
 */
export default function ServicePage() {
  const params = useParams<{ name: string }>();
  const range = useRange();
  const navigate = useNavigate();
  // The operation or the call open in the modal, in the URL, so Back closes
  // it and a link opens it. A call has the type, the system, and the name of
  // its target too.
  const [modal, setParams] = useSearchParams<{
    op?: string;
    kind?: string;
    call?: string;
    system?: string;
    target?: string;
  }>();
  const openOperation = createMemo(
    () => {
      const kind = Number(modal.kind);
      return modal.op !== undefined && Number.isInteger(kind)
        ? { name: modal.op, kind, target: parseTarget(modal.call, modal.system, modal.target) }
        : undefined;
    },
    undefined,
    {
      equals: (a, b) =>
        a?.name === b?.name &&
        a?.kind === b?.kind &&
        (a?.target === undefined || b?.target === undefined
          ? a?.target === b?.target
          : sameTarget(a.target, b.target)),
    },
  );
  const name = () => params.name;
  const term = () => `service = ${quote(name())}`;

  const fetched = createRangeFetch(
    range,
    () => ({ name: name() }),
    ({ name: service, ...query }, signal) => getService(service, query, signal),
  );
  const calls = createRangeFetch(
    range,
    () => ({ name: name() }),
    ({ name: service, ...query }, signal) => getCalls(service, query, signal),
  );
  const errorTraces = createRangeFetch(
    range,
    () => ({ q: `${term()} error = true`, limit: 10 }),
    getTraces,
  );
  const errorLogs = createRangeFetch(
    range,
    () => ({ q: `${term()} level >= error`, limit: 10 }),
    getLogGroups,
  );

  const data = () => {
    const service = fetched.data();
    return service?.service === name() ? service : undefined;
  };
  const tracesLink = (q: string) => `/traces${range.search({ q })}`;
  const logsLink = (q: string, view?: string) => `/logs${range.search({ q, view })}`;
  const zoom = (start: number, end: number) =>
    range.setRange(new Date(start).toISOString(), new Date(end).toISOString());

  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <Toolbar
        range={range}
        fetched={fetched}
        title={
          <div class="flex min-w-0 items-center gap-3">
            <A href={`/services${range.search()}`} class={link}>
              Services
            </A>
            <span class="text-muted">/</span>
            <h1 class="m-0 min-w-0 font-mono text-md font-semibold">
              <Service name={name()} resource={data()?.resource ?? {}} />
            </h1>
          </div>
        }
      >
        <span class="ml-3 flex gap-3">
          <A href={tracesLink(term())} class={link}>
            Traces
          </A>
          <A href={logsLink(term())} class={link}>
            Logs
          </A>
        </span>
      </Toolbar>

      <div class={`min-h-0 flex-1 ${pageContent}`}>
        <Show when={fetched.error()}>
          {(error) => (
            <div class="mb-3">
              <Callout tone="error">{error()}</Callout>
            </div>
          )}
        </Show>
        <Show when={data()}>
          {(service) => (
            <Show
              when={service().stats.requests.count > 0 || service().stats.logs > 0}
              fallback={<Empty>{name()} sent no spans or logs in this range.</Empty>}
            >
              <div class="flex flex-col gap-4">
                <Overview
                  service={service()}
                  loading={fetched.loading()}
                  onZoom={zoom}
                  operationHref={(operation) =>
                    `/services/${encodeURIComponent(name())}${range.search({
                      op: operation.name,
                      kind: String(operation.kind),
                    })}`
                  }
                  onOpenOperation={(operation) =>
                    setParams({ ...closed, op: operation.name, kind: String(operation.kind) })
                  }
                />

                <Show when={calls.data()}>
                  {(body) => (
                    <Show when={body().service === name() && body().calls.count > 0}>
                      <CallsSection
                        calls={body()}
                        loading={calls.loading()}
                        onZoom={zoom}
                        callHref={(row) =>
                          `/services/${encodeURIComponent(name())}${range.search(callParams(row))}`
                        }
                        onOpenCall={(row) => setParams(callParams(row))}
                      />
                    </Show>
                  )}
                </Show>

                <Panel
                  title="Failed traces"
                  description="The newest traces with a failed span of the service"
                  flush
                  actions={
                    <A href={tracesLink(`${term()} error = true`)} class={link}>
                      All failed traces
                    </A>
                  }
                >
                  <Show when={errorTraces.data()}>
                    {(body) => (
                      <Show
                        when={body().traces.length > 0}
                        fallback={<Empty>No trace of {name()} failed in this range.</Empty>}
                      >
                        <TraceList
                          traces={body().traces}
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
                    <A href={logsLink(`${term()} level >= error`, "groups")} class={link}>
                      All error logs
                    </A>
                  }
                >
                  <Show when={errorLogs.data()}>
                    {(body) => (
                      <Show
                        when={body().groups.length > 0}
                        fallback={<Empty>{name()} logged no errors in this range.</Empty>}
                      >
                        <LogGroupList
                          groups={body().groups}
                          onShowLines={(shown) =>
                            navigate(logsLink(addTerm(`${term()} level >= error`, shown)))
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

      <Show when={openOperation()} keyed>
        {(operation) => (
          <OperationModal
            service={name()}
            name={operation.name}
            kind={operation.kind}
            target={operation.target}
            range={range}
            onClose={() => setParams(closed)}
          />
        )}
      </Show>
    </div>
  );
}

/**
 * The numbers of the range, their charts over time, and the requests by
 * operation. Each series is built once per answer, since a prop written as
 * an array literal would build it again on every read.
 */
function Overview(props: {
  service: ServiceBody;
  loading: boolean;
  onZoom: (start: number, end: number) => void;
  operationHref: (operation: Operation) => string;
  onOpenOperation: (operation: Operation) => void;
}) {
  const frame = createMemo<TimeFrame>(() => timeFrame(props.service));
  const buckets = () => props.service.buckets;
  const stats = () => props.service.stats;
  const requests = () => stats().requests;
  const rangeSeconds = () => seconds(props.service.start_at, props.service.end_at);

  const requestChart = createMemo(() => requestSeries(buckets()));
  const latencyChart = createMemo(() => latencySeries(buckets()));
  const errorRateChart = createMemo(() => errorRateSeries(buckets()));
  const logSeries = createMemo<TimeSeries[]>(() => [
    { label: "Below error", color: "muted", values: buckets().map((b) => b.logs - b.error_logs) },
    { label: "Error and above", color: "error", values: buckets().map((b) => b.error_logs) },
  ]);

  const operationColumns = createMemo<Column<Operation>[]>(() => [
    {
      id: "name",
      label: "Operation",
      value: (o) => o.name,
      cell: (o) => (
        <SpanTitle name={o.name} attributes={o.attributes} error={false} status={false} />
      ),
    },
    {
      id: "kind",
      label: "Kind",
      width: "max-content",
      value: (o) => o.kind,
      cell: (o) => <KindBadge kind={o.kind} />,
    },
    {
      id: "requests",
      label: "Requests",
      unit: "count",
      meter: true,
      value: (o) => o.requests.count,
    },
    {
      id: "rate",
      label: "Rate",
      unit: "rate",
      value: (o) => rate(o.requests.count, rangeSeconds()),
    },
    {
      id: "errors",
      label: "Error rate",
      unit: "ratio",
      value: (o) => share(o.requests.errors, o.requests.count),
      tone: (o) => (o.requests.errors > 0 ? "error" : undefined),
    },
    ...PERCENTILES.map((p): Column<Operation> => ({
      id: p,
      label: p.toUpperCase(),
      unit: "duration",
      value: (o) => o.requests.latency?.[p] ?? null,
    })),
    {
      id: "total",
      label: "Total time",
      description: "The durations of its requests added up",
      unit: "duration",
      meter: true,
      value: (o) => o.requests.total_ns,
    },
  ]);

  return (
    <>
      <div class="grid grid-cols-[repeat(auto-fit,minmax(170px,1fr))] gap-3">
        <Stat
          label="Requests"
          value={requests().count}
          unit="count"
          detail={formatValue(rate(requests().count, rangeSeconds()), "rate")}
          trend={buckets().map((b) => b.requests.count)}
        />
        <Stat
          label="Error rate"
          value={share(requests().errors, requests().count)}
          unit="ratio"
          tone={requests().errors > 0 ? "error" : "default"}
          detail={`${requests().errors.toLocaleString()} failed`}
          trend={errorRateChart()[0]?.values}
          trendColor="error"
        />
        <For each={PERCENTILES}>
          {(p, i) => (
            <Stat
              label={`${p.toUpperCase()} latency`}
              value={requests().latency?.[p]}
              unit="duration"
              trend={latencyChart()[i()]?.values}
              trendColor={p}
            />
          )}
        </For>
        <Stat
          label="Logs"
          value={stats().logs}
          unit="count"
          detail={`${stats().error_logs.toLocaleString()} errors`}
          trend={buckets().map((b) => b.logs)}
          trendColor="muted"
        />
      </div>

      <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <ChartPanel
          title="Requests"
          description="Spans that enter the service, by step"
          kind="bar"
          unit="count"
          frame={frame()}
          series={requestChart()}
          loading={props.loading}
          onZoom={props.onZoom}
        />
        <ChartPanel
          title="Latency"
          description="Percentiles of the durations of the requests"
          kind="line"
          unit="duration"
          frame={frame()}
          series={latencyChart()}
          loading={props.loading}
          onZoom={props.onZoom}
          empty="No requests in this range"
        />
        <ChartPanel
          title="Error rate"
          description="The share of the requests that failed"
          kind="area"
          unit="ratio"
          frame={frame()}
          series={errorRateChart()}
          loading={props.loading}
          onZoom={props.onZoom}
          empty="No requests in this range"
        />
        <ChartPanel
          title="Logs"
          description="Log lines by level, by step"
          kind="bar"
          unit="count"
          frame={frame()}
          series={logSeries()}
          loading={props.loading}
          onZoom={props.onZoom}
        />
      </div>

      <Show when={props.service.operations.length > 0}>
        <Panel
          title="Operations"
          description={
            props.service.truncated
              ? "The requests by span name, the most requested only"
              : "The requests by span name"
          }
          flush
        >
          <Table
            label="Operations"
            rows={props.service.operations}
            columns={operationColumns()}
            sort={{ column: "requests", descending: true }}
            href={props.operationHref}
            onRowClick={props.onOpenOperation}
            loading={props.loading}
          />
        </Panel>
      </Show>
    </>
  );
}
