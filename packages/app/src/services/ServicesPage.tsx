import { Show } from "solid-js";

import { getServices, type ServiceSummary } from "@siner/api";
import { Callout } from "@siner/ui";
import { type Column, Panel, Sparkline, Table } from "@siner/viz";

import { pageContent } from "../classes";
import { count } from "../list";
import Service from "../Service";
import { createRangeFetch, useRange } from "./range";
import { rate, seconds, share } from "./stats";
import Toolbar from "./Toolbar";

/**
 * Every service that sent spans or logs in the range, with its requests over
 * time, its rate, its error rate, its latency, and its logs. A request is a
 * span that enters the service: a root span, or a server or consumer span. A
 * service opens its own page. The range and live mode live in the URL.
 */
export default function ServicesPage() {
  const range = useRange();
  const fetched = createRangeFetch(range, () => ({}), getServices);
  const rangeSeconds = () => {
    const data = fetched.data();
    return data ? seconds(data.since, data.until) : 0;
  };

  const columns: Column<ServiceSummary>[] = [
    {
      id: "service",
      label: "Service",
      value: (s) => s.service,
      cell: (s) => <Service name={s.service} resource={s.resource} />,
    },
    {
      id: "traffic",
      label: "Requests over time",
      value: () => null,
      sortable: false,
      width: "max-content",
      cell: (s) => (
        <Sparkline
          kind="bar"
          series={[
            {
              values: s.buckets.map((b) => b.requests.count - b.requests.errors),
              color: "series-1",
            },
            { values: s.buckets.map((b) => b.requests.errors), color: "error" },
          ]}
        />
      ),
    },
    { id: "requests", label: "Requests", unit: "count", value: (s) => s.stats.requests.count },
    {
      id: "rate",
      label: "Rate",
      unit: "rate",
      value: (s) => rate(s.stats.requests.count, rangeSeconds()),
    },
    {
      id: "errors",
      label: "Error rate",
      unit: "ratio",
      value: (s) => share(s.stats.requests.errors, s.stats.requests.count),
      tone: (s) => (s.stats.requests.errors > 0 ? "error" : undefined),
    },
    ...(["p50", "p95", "p99"] as const).map((p): Column<ServiceSummary> => ({
      id: p,
      label: p.toUpperCase(),
      unit: "duration",
      value: (s) => s.stats.requests.latency?.[p] ?? null,
    })),
    { id: "logs", label: "Logs", unit: "count", value: (s) => s.stats.logs },
    {
      id: "error_logs",
      label: "Error logs",
      unit: "count",
      value: (s) => s.stats.error_logs,
      tone: (s) => (s.stats.error_logs > 0 ? "error" : "muted"),
    },
  ];

  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <Toolbar
        range={range}
        fetched={fetched}
        title={<h1 class="m-0 font-mono text-md font-semibold">Services</h1>}
      >
        <Show when={fetched.data()}>
          {(data) => (
            <span>
              {count(data().services.length, "service")}
              {data().truncated ? ", the busiest; more sent telemetry" : ""}
            </span>
          )}
        </Show>
      </Toolbar>

      <div class={`min-h-0 flex-1 ${pageContent}`}>
        <Show when={fetched.error()}>
          {(error) => (
            <div class="mb-3">
              <Callout tone="error">{error()}</Callout>
            </div>
          )}
        </Show>
        <Show when={fetched.data()}>
          {(data) => (
            <Panel flush>
              <Table
                label="Services"
                rows={data().services}
                columns={columns}
                sort={{ column: "requests", descending: true }}
                href={(s) => `/services/${encodeURIComponent(s.service)}${range.search()}`}
                loading={fetched.loading()}
                empty="No service sent spans or logs in this range."
              />
            </Panel>
          )}
        </Show>
      </div>
    </div>
  );
}
