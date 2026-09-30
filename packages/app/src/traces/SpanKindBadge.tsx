import { Badge, type Tone } from "@otelo/ui";

const KIND_TONES: Record<SpanKindName, Tone> = {
  unspecified: "trace",
  internal: "muted",
  server: "info",
  client: "placeholder",
  producer: "warn",
  consumer: "success",
};

type SpanKindName = "unspecified" | "internal" | "server" | "client" | "producer" | "consumer";

interface SpanKindBadgeProps {
  readonly kind: string;
}

export default function SpanKindBadge(props: SpanKindBadgeProps) {
  return (
    <Badge tone={isSpanKindName(props.kind) ? KIND_TONES[props.kind] : "trace"}>{props.kind}</Badge>
  );
}

function isSpanKindName(kind: string): kind is SpanKindName {
  return Object.hasOwn(KIND_TONES, kind);
}
