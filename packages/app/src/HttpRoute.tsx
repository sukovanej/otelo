import { For } from "solid-js";

import { type RoutePartKind, splitRouteIntoParts } from "./semantics";

const ROUTE_PART_CLASSES: Record<RoutePartKind, string> = {
  slash: "text-muted",
  parameter: "text-placeholder",
  text: "",
};

interface HttpRouteProps {
  readonly route: string;
}

export default function HttpRoute(props: HttpRouteProps) {
  return (
    <span class="truncate" title={props.route}>
      <For each={splitRouteIntoParts(props.route)} keyed={false}>
        {(part) => <span class={ROUTE_PART_CLASSES[part().kind]}>{part().text}</span>}
      </For>
    </span>
  );
}
