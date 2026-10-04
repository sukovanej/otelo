import type { JSX } from "@solidjs/web";
import { createMemo, createSignal, Show } from "solid-js";

import Panel from "../Panel";
import type { ChartKind, TimeFrame, TimeSeries } from "../series";
import type { Unit } from "../units";
import ChartPanelLegend from "./chart-panel-legend";
import ChartPanelPlot from "./chart-panel-plot";

type LegendPlacement = "header" | "footer";

interface ChartPanelProps {
  readonly title: string;
  readonly description: JSX.Element;
  readonly frame: TimeFrame;
  readonly series: ReadonlyArray<TimeSeries>;
  readonly kind: ChartKind;
  readonly unit: Unit;
  readonly loading: boolean;
  readonly emptyMessage?: string | undefined;
  readonly actions?: JSX.Element;
  readonly legendPlacement?: LegendPlacement;
  readonly fill?: boolean;
  readonly singleLineHeader?: boolean;
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
  const legend = () => (
    <Show when={props.series.length > 1}>
      <ChartPanelLegend
        series={props.series}
        kind={props.kind}
        isolatedIndex={isolatedIndex()}
        onIsolate={isolateSeries}
      />
    </Show>
  );
  const legendInFooter = () => props.legendPlacement === "footer";
  return (
    <Panel
      title={props.title}
      description={props.description}
      fill={props.fill}
      singleLineHeader={props.singleLineHeader}
      actions={
        <>
          <Show when={!legendInFooter()}>{legend()}</Show>
          {props.actions}
        </>
      }
    >
      <div class={["pt-3", { "flex min-h-0 flex-1 flex-col": props.fill }]}>
        <ChartPanelPlot
          label={props.title}
          frame={props.frame}
          series={props.series}
          kind={props.kind}
          unit={props.unit}
          isolatedIndex={isolatedIndex()}
          loading={props.loading}
          emptyMessage={props.emptyMessage}
          height={props.fill ? "fill" : "fixed"}
          onZoom={props.onZoom}
        />
      </div>
      <Show when={legendInFooter()}>
        <div class="-mx-1.5 max-h-16 shrink-0 overflow-y-auto pt-1">{legend()}</div>
      </Show>
    </Panel>
  );
}
