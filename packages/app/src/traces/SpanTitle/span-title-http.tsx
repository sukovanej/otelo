import { Show } from "solid-js";

import { GlobeIcon } from "@otelo/icons";
import { Tooltip } from "@otelo/ui";

import HttpMethodBadge from "../../HttpMethodBadge";
import HttpRoute from "../../HttpRoute";
import HttpStatusBadge from "../../HttpStatusBadge";
import type { HttpSpan } from "../../semantics";

interface SpanTitleHttpProps {
  readonly meaning: HttpSpan;
  readonly remainingName: string;
}

export default function SpanTitleHttp(props: SpanTitleHttpProps) {
  return (
    <>
      <Tooltip content="HTTP request" class="self-center">
        <GlobeIcon class="text-muted" title="HTTP request" />
      </Tooltip>
      <Show when={props.meaning.method}>{(method) => <HttpMethodBadge method={method()} />}</Show>
      <Show when={props.meaning.route}>{(route) => <HttpRoute route={route()} />}</Show>
      <Show when={props.meaning.status}>{(status) => <HttpStatusBadge status={status()} />}</Show>
      <Show when={props.remainingName}>
        <span class="truncate text-muted">{props.remainingName}</span>
      </Show>
    </>
  );
}
