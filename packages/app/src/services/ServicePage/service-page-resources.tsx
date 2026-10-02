import { createMemo, Match, Show, Switch } from "solid-js";

import { Callout, EmptyMessage } from "@otelo/ui";
import { ChartPanel, Panel } from "@otelo/viz";

import FetchErrorBoundary from "../../FetchErrorBoundary";
import { createRangeFetch, type RangeState } from "../range";
import { getServiceResources, RESOURCE_METRIC_NAMES, toResourceCharts } from "../resources";

interface ServicePageResourcesProps {
  readonly service: string;
  readonly range: RangeState;
  readonly onZoom: (start: number, end: number) => void;
}

export default function ServicePageResources(props: ServicePageResourcesProps) {
  const fetchedResources = createRangeFetch(
    props.range,
    "resources",
    () => ({ service: props.service }),
    getServiceResources,
  );
  const charts = createMemo(() => {
    const answer = fetchedResources.data();
    return answer?.service === props.service ? toResourceCharts(answer) : undefined;
  });
  const shownCharts = () => {
    const resourceCharts = charts();
    return resourceCharts?.state === "shown" ? resourceCharts : undefined;
  };

  return (
    <>
      <Show when={fetchedResources.errorMessage()}>
        {(errorMessage) => <Callout tone="error">{errorMessage()}</Callout>}
      </Show>
      <FetchErrorBoundary>
        <Switch>
          <Match when={charts()?.state === "missing"}>
            <Panel title="CPU and memory" description="From the host collector">
              <EmptyMessage>
                The host collector sent no{" "}
                <span class="font-mono">{RESOURCE_METRIC_NAMES.cpuTime}</span>,{" "}
                <span class="font-mono">{RESOURCE_METRIC_NAMES.memory}</span>, or{" "}
                <span class="font-mono">{RESOURCE_METRIC_NAMES.cgroupMemory}</span> of{" "}
                {props.service} in this range. It names them after the systemd unit or the launchd
                job, so a service whose unit has another name has none.
              </EmptyMessage>
            </Panel>
          </Match>
          <Match when={shownCharts()}>
            {(shown) => (
              <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
                <ChartPanel
                  title="CPU"
                  description="The CPU time of its processes as a share of one core"
                  kind="area"
                  unit="ratio"
                  frame={shown().frame}
                  series={shown().cpuSeries}
                  loading={fetchedResources.loading()}
                  onZoom={props.onZoom}
                  emptyMessage="No CPU time in this range"
                />
                <ChartPanel
                  title="Memory"
                  description="The memory of its processes, and on Linux of its cgroup with the page cache"
                  kind="line"
                  unit="bytes"
                  frame={shown().frame}
                  series={shown().memorySeries}
                  loading={fetchedResources.loading()}
                  onZoom={props.onZoom}
                  emptyMessage="No memory in this range"
                />
              </div>
            )}
          </Match>
        </Switch>
      </FetchErrorBoundary>
    </>
  );
}
