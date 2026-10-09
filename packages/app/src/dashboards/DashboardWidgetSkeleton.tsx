import type { JSX } from "@solidjs/web";

import { ChartSkeleton, Panel } from "@otelo/viz";

interface DashboardWidgetSkeletonProps {
  readonly title: string;
  readonly actions?: JSX.Element | undefined;
}

export default function DashboardWidgetSkeleton(props: DashboardWidgetSkeletonProps) {
  return (
    <Panel title={props.title} actions={props.actions} fill singleLineHeader actionsOnHover>
      <ChartSkeleton />
    </Panel>
  );
}
