import { Show } from "solid-js";

import { GlobeIcon } from "@otelo/icons";
import { Badge, type Tone, Tooltip } from "@otelo/ui";

import type { HttpSpan } from "../../semantics";
import SpanTitleRoute from "./span-title-route";

const METHOD_TONES: Record<string, Tone> = {
  GET: "info",
  POST: "success",
  PUT: "warn",
  PATCH: "warn",
  DELETE: "error",
};

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
      <Show when={props.meaning.method}>
        {(method) => <Badge tone={METHOD_TONES[method()] ?? "muted"}>{method()}</Badge>}
      </Show>
      <Show when={props.meaning.route}>{(route) => <SpanTitleRoute route={route()} />}</Show>
      <Show when={props.meaning.status}>
        {(status) => <Badge tone={toStatusTone(status())}>{status()}</Badge>}
      </Show>
      <Show when={props.remainingName}>
        <span class="truncate text-muted">{props.remainingName}</span>
      </Show>
    </>
  );
}

function toStatusTone(status: number): Tone {
  if (status >= 500) return "error";
  if (status >= 400) return "warn";
  if (status >= 300) return "info";
  if (status >= 200) return "success";
  return "muted";
}
