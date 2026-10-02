import { Errored, Show } from "solid-js";

import { getServices, type ServiceSummary } from "@otelo/api";
import { Callout } from "@otelo/ui";
import { type Column, Panel, Sparkline, Table } from "@otelo/viz";

import { pageContent } from "../classes";
import { formatCount } from "../count";
import FetchErrorBoundary from "../FetchErrorBoundary";
import ServiceName from "../ServiceName";
import { createRangeFetch, useRange } from "./range";
import RangeBar from "./range-bar";
import { measureSeconds, toRate, toShare } from "./stats";

export default function ServicesPage() {
  const range = useRange();
  const fetched = createRangeFetch(range, "services", () => ({}), getServices);
  const rangeSeconds = () => {
    const services = fetched.data();
    return services ? measureSeconds(services.start_at, services.end_at) : 0;
  };

  const columns: Column<ServiceSummary>[] = [
    {
      kind: "cell",
      id: "service",
      label: "Service",
      sortBy: (service) => service.service,
      cell: (service) => <ServiceName name={service.service} resource={service.resource} />,
    },
    {
      kind: "cell",
      id: "traffic",
      label: "Requests over time",
      width: "max-content",
      cell: (service) => (
        <Sparkline
          kind="bar"
          series={[
            {
              values: service.buckets.map(
                (bucket) => bucket.requests.count - bucket.requests.errors,
              ),
              color: "series-1",
            },
            { values: service.buckets.map((bucket) => bucket.requests.errors), color: "error" },
          ]}
        />
      ),
    },
    {
      kind: "number",
      id: "requests",
      label: "Requests",
      unit: "count",
      value: (service) => service.stats.requests.count,
    },
    {
      kind: "number",
      id: "rate",
      label: "Rate",
      unit: "rate",
      value: (service) => toRate(service.stats.requests.count, rangeSeconds()),
    },
    {
      kind: "number",
      id: "errors",
      label: "Error rate",
      unit: "ratio",
      value: (service) => toShare(service.stats.requests.errors, service.stats.requests.count),
      tone: (service) => (service.stats.requests.errors > 0 ? "error" : undefined),
    },
    ...(["p50", "p95", "p99"] as const).map((percentile): Column<ServiceSummary> => ({
      kind: "number",
      id: percentile,
      label: percentile.toUpperCase(),
      unit: "duration",
      value: (service) => service.stats.requests.latency?.[percentile] ?? null,
    })),
    {
      kind: "number",
      id: "logs",
      label: "Logs",
      unit: "count",
      value: (service) => service.stats.logs,
    },
    {
      kind: "number",
      id: "error_logs",
      label: "Error logs",
      unit: "count",
      value: (service) => service.stats.error_logs,
      tone: (service) => (service.stats.error_logs > 0 ? "error" : "muted"),
    },
  ];

  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <RangeBar
        range={range}
        fetched={fetched}
        title={<h1 class="m-0 font-mono text-md font-semibold">Services</h1>}
      >
        <Errored fallback={null}>
          <Show when={fetched.data()}>
            {(services) => (
              <span>
                {formatCount(services().services.length, "service")}
                {services().truncated ? ", the busiest; more sent telemetry" : ""}
              </span>
            )}
          </Show>
        </Errored>
      </RangeBar>

      <div class={`min-h-0 flex-1 ${pageContent}`}>
        <Show when={fetched.errorMessage()}>
          {(errorMessage) => (
            <div class="mb-3">
              <Callout tone="error">{errorMessage()}</Callout>
            </div>
          )}
        </Show>
        <FetchErrorBoundary>
          <Show when={fetched.data()}>
            {(services) => (
              <Panel flush>
                <Table
                  label="Services"
                  rows={services().services}
                  columns={columns}
                  initialSort={{ columnId: "requests", descending: true }}
                  href={(service) =>
                    `/services/${encodeURIComponent(service.service)}${range.toSearch()}`
                  }
                  loading={fetched.loading()}
                  emptyMessage="No service sent spans or logs in this range."
                />
              </Panel>
            )}
          </Show>
        </FetchErrorBoundary>
      </div>
    </div>
  );
}
