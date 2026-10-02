import type { JSX } from "@solidjs/web";
import { createMemo, createSignal, Show } from "solid-js";

import Panel from "../Panel";
import type { ChartKind, TimeFrame, TimeSeries } from "../series";
import type { Unit } from "../units";
import ChartPanelLegend from "./chart-panel-legend";
import ChartPanelPlot from "./chart-panel-plot";

interface ChartPanelProps {
  readonly title: string;
  readonly description: JSX.Element;
  readonly frame: TimeFrame;
  readonly series: ReadonlyArray<TimeSeries>;
  readonly kind: ChartKind;
  readonly unit: Unit;
  readonly loading: boolean;
  readonly emptyMessage?: string | undefined;
  readonly onZoom: (startMs: number, endMs: number) => void;
}

export default function ChartPanel(props: ChartPanelProps) {
  const [isolatedLabel, setIsolatedLabel] = createSignal<string>();
  const isolatedIndex = createMemo(() => {
    const index = props.series.findIndex((series) => series.label === isolatedLabel());
    return index === -1 ? undefined : index;
  });
  const isolateSeries = (index: number | undefined) =>
    setIsolatedLabel(index === undefined ? undefined : props.series[index]?.label);
  return (
    <Panel
      title={props.title}
      description={props.description}
      actions={
        <Show when={props.series.length > 1}>
          <ChartPanelLegend
            series={props.series}
            kind={props.kind}
            isolatedIndex={isolatedIndex()}
            onIsolate={isolateSeries}
          />
        </Show>
      }
    >
      <div class="pt-3">
        <ChartPanelPlot
          label={props.title}
          frame={props.frame}
          series={props.series}
          kind={props.kind}
          unit={props.unit}
          isolatedIndex={isolatedIndex()}
          loading={props.loading}
          emptyMessage={props.emptyMessage}
          onZoom={props.onZoom}
        />
      </div>
    </Panel>
  );
}
