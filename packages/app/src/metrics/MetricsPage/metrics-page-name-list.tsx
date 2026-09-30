import { A } from "@solidjs/router";
import { For, Show } from "solid-js";

import { EmptyMessage } from "@otelo/ui";

import type { MetricName } from "../metric";

interface MetricsPageNameListProps {
  readonly names: ReadonlyArray<MetricName>;
  readonly openName: string | undefined;
  readonly loaded: boolean;
  readonly toHref: (name: string) => string;
}

export default function MetricsPageNameList(props: MetricsPageNameListProps) {
  return (
    <nav
      aria-label="Metrics"
      class="w-80 shrink-0 overflow-y-auto border-r border-line bg-surface py-2"
    >
      <Show
        when={props.names.length > 0 || !props.loaded}
        fallback={<EmptyMessage>No metrics in this range match the query.</EmptyMessage>}
      >
        <ul class="m-0 list-none p-0">
          <For each={props.names}>
            {(metric) => (
              <li>
                <A
                  href={props.toHref(metric.name)}
                  class="block px-4 py-1.5 hover:bg-hover"
                  classList={{ "bg-active": metric.name === props.openName }}
                  aria-current={metric.name === props.openName ? "page" : undefined}
                >
                  <div class="truncate font-mono text-ink" title={metric.name}>
                    {metric.name}
                  </div>
                  <div class="truncate text-xs text-muted">
                    {[
                      metric.kinds.join(", "),
                      ...metric.units.filter((unit) => unit !== ""),
                      `${metric.seriesCount.toLocaleString()} series`,
                    ].join(" · ")}
                  </div>
                </A>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </nav>
  );
}
