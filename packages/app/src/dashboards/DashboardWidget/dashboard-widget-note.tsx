import type { JSX } from "@solidjs/web";

import { Panel } from "@otelo/viz";

import { WIDGET_BODY_CLASSES } from "./dashboard-widget-body";

interface DashboardWidgetNoteProps {
  readonly title: string;
  readonly text: string;
  readonly actions: JSX.Element | undefined;
}

export default function DashboardWidgetNote(props: DashboardWidgetNoteProps) {
  return (
    <Panel title={props.title} actions={props.actions} fill singleLineHeader actionsOnHover>
      <div class={`${WIDGET_BODY_CLASSES} overflow-y-auto whitespace-pre-wrap wrap-anywhere`}>
        {props.text}
      </div>
    </Panel>
  );
}
