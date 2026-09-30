import { For } from "solid-js";

import { type RoutePartKind, splitRouteIntoParts } from "../../semantics";

const ROUTE_PART_CLASSES: Record<RoutePartKind, string> = {
  slash: "text-muted",
  parameter: "text-placeholder",
  text: "",
};

interface SpanTitleRouteProps {
  readonly route: string;
}

export default function SpanTitleRoute(props: SpanTitleRouteProps) {
  return (
    <span class="truncate" title={props.route}>
      <For each={splitRouteIntoParts(props.route)}>
        {(part) => <span class={ROUTE_PART_CLASSES[part.kind]}>{part.text}</span>}
      </For>
    </span>
  );
}
