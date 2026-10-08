import type { JSX } from "@solidjs/web";
import { Errored, Match, Show, Switch } from "solid-js";

import type { Widget } from "@otelo/api";
import { Callout } from "@otelo/ui";
import { ChartPanel, Panel } from "@otelo/viz";

import { describeError, freezeDeeply } from "../../fetch";
import { createRangeFetch, type RangeState } from "../../services/range";
import { type FetchedDisplay, fetchWidgetData, type WidgetData } from "../fetch-widget";
import type { MeasuredChart } from "../measure";
import { describeWidgetTitle } from "../widget";
import { WIDGET_BODY_CLASSES } from "./dashboard-widget-body";
import DashboardWidgetGroupTitle from "./dashboard-widget-group-title";
import DashboardWidgetToplist from "./dashboard-widget-toplist";
import DashboardWidgetValue from "./dashboard-widget-value";

interface DashboardWidgetDataProps {
  readonly widget: Widget;
  readonly display: FetchedDisplay;
  readonly range: Pick<RangeState, "since" | "until" | "live" | "setRange">;
  readonly actions: JSX.Element | undefined;
}

export default function DashboardWidgetData(props: DashboardWidgetDataProps) {
  const fetched = createRangeFetch(
    props.range,
    "dashboard-widget",
    () => ({ display: props.display }),
    ({ display, since, until }, signal) =>
      fetchWidgetData(display, { since, until }, signal).then(freezeDeeply),
  );
  const title = () => describeWidgetTitle(props.widget);
  const chartKind = () =>
    props.widget.display.kind === "timeseries" ? props.widget.display.chart : "line";
  const zoomRangeTo = (startMs: number, endMs: number) =>
    props.range.setRange(new Date(startMs).toISOString(), new Date(endMs).toISOString());
  const drawMessage = (message: JSX.Element) => (
    <Panel title={title()} actions={props.actions} fill singleLineHeader actionsOnHover>
      <div class={`${WIDGET_BODY_CLASSES} justify-center text-center text-muted`}>{message}</div>
    </Panel>
  );

  return (
    <Errored
      fallback={(error) => drawMessage(<Callout tone="error">{describeError(error())}</Callout>)}
    >
      <Switch fallback={drawMessage("Loading…")}>
        <Match when={fetched.errorMessage()}>
          {(errorMessage) => drawMessage(<Callout tone="error">{errorMessage()}</Callout>)}
        </Match>
        <Match when={readChart(fetched.data())}>
          {(chart) => (
            <ChartPanel
              title={title()}
              description={describeChart(chart())}
              frame={chart().frame}
              series={chart().series}
              kind={chartKind()}
              unit={chart().unit}
              loading={fetched.loading()}
              emptyMessage="Nothing in this range matches."
              actions={props.actions}
              legendPlacement="footer"
              fill
              singleLineHeader
              actionsOnHover
              drawSeriesLabel={(series) => (
                <DashboardWidgetGroupTitle label={series.label} groupKey={series.groupKey} />
              )}
              onZoom={zoomRangeTo}
            />
          )}
        </Match>
        <Match when={readMeasured(fetched.data())}>
          {(measured) => (
            <Panel title={title()} actions={props.actions} fill singleLineHeader actionsOnHover>
              <div class={[WIDGET_BODY_CLASSES, { "opacity-60": fetched.loading() }]}>
                <Show
                  when={props.widget.display.kind === "toplist"}
                  fallback={<DashboardWidgetValue measured={measured()} />}
                >
                  <DashboardWidgetToplist measured={measured()} />
                </Show>
              </div>
            </Panel>
          )}
        </Match>
        <Match when={readReason(fetched.data())} keyed>
          {(reason) => drawMessage(reason)}
        </Match>
      </Switch>
    </Errored>
  );
}

function describeChart(chart: MeasuredChart): string {
  const leftOut = chart.otherUnitQueryCount;
  return [
    chart.truncated ? "The top groups" : "",
    leftOut > 0
      ? `Leaves out ${leftOut} ${leftOut === 1 ? "query" : "queries"} in another unit`
      : "",
  ]
    .filter((part) => part !== "")
    .join(" · ");
}

function readChart(data: WidgetData | undefined) {
  return data?.kind === "chart" ? data.chart : undefined;
}

function readMeasured(data: WidgetData | undefined) {
  return data?.kind === "measured" ? data.measured : undefined;
}

function readReason(data: WidgetData | undefined) {
  return data?.kind === "none" ? data.reason : undefined;
}
