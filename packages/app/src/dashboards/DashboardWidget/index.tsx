import type { JSX } from "@solidjs/web";
import { Show } from "solid-js";

import type { Widget } from "@otelo/api";

import type { RangeState } from "../../services/range";
import { toFetchedDisplay } from "../fetch-widget";
import DashboardWidgetData from "./dashboard-widget-data";
import DashboardWidgetNote from "./dashboard-widget-note";

interface DashboardWidgetProps {
  readonly widget: Widget;
  readonly range: Pick<RangeState, "since" | "until" | "live" | "setRange">;
  readonly actions?: JSX.Element;
}

export default function DashboardWidget(props: DashboardWidgetProps) {
  const note = () => (props.widget.display.kind === "note" ? props.widget.display : undefined);
  return (
    <Show
      when={toFetchedDisplay(props.widget.display)}
      fallback={
        <Show when={note()}>
          {(shownNote) => (
            <DashboardWidgetNote
              title={props.widget.title}
              text={shownNote().text}
              actions={props.actions}
            />
          )}
        </Show>
      }
    >
      {(display) => (
        <DashboardWidgetData
          widget={props.widget}
          display={display()}
          range={props.range}
          actions={props.actions}
        />
      )}
    </Show>
  );
}
