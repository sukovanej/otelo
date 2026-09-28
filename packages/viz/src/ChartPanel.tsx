import { createSignal, type JSX, Show } from "solid-js";

import Legend from "./Legend";
import Panel from "./Panel";
import TimeSeriesChart from "./TimeSeriesChart";

/**
 * A chart in a panel: its title and a line under it on the left, the legend
 * on the right when there are two series or more, and the chart under them.
 * It takes every prop of `TimeSeriesChart`, and the legend works the same.
 */
export default function ChartPanel(
  props: Omit<Parameters<typeof TimeSeriesChart>[0], "legend" | "isolated" | "onIsolate"> & {
    title: string;
    description?: JSX.Element;
    class?: string;
  },
) {
  const [isolated, setIsolated] = createSignal<number>();
  return (
    <Panel
      title={props.title}
      description={props.description}
      class={props.class}
      actions={
        <Show when={props.series.length > 1}>
          <Legend
            series={props.series}
            kind={props.kind}
            isolated={isolated()}
            onIsolate={setIsolated}
          />
        </Show>
      }
    >
      <div class="pt-3">
        <TimeSeriesChart
          {...props}
          label={props.label ?? props.title}
          legend={false}
          isolated={isolated()}
          onIsolate={setIsolated}
        />
      </div>
    </Panel>
  );
}
