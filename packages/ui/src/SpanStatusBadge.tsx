import Badge, { type BadgeTone } from "./Badge";

const STATUS_TONES: Record<SpanStatusName, BadgeTone> = {
  unset: "muted",
  ok: "success",
  error: "error",
};

type SpanStatusName = "unset" | "ok" | "error";

interface SpanStatusBadgeProps {
  readonly status: string;
}

export default function SpanStatusBadge(props: SpanStatusBadgeProps) {
  return (
    <Badge tone={isSpanStatusName(props.status) ? STATUS_TONES[props.status] : "muted"}>
      {props.status}
    </Badge>
  );
}

function isSpanStatusName(status: string): status is SpanStatusName {
  return Object.hasOwn(STATUS_TONES, status);
}
