import { Badge, type Tone } from "@siner/ui";
import { kindName } from "./span";

const tones: Record<string, Tone> = {
  server: "info",
  client: "placeholder",
  internal: "muted",
  producer: "warn",
  consumer: "success",
};

/** The OpenTelemetry kind of a span, in a color of its own. */
export default function KindBadge(props: { kind: number }) {
  return <Badge tone={tones[kindName(props.kind)] ?? "trace"}>{kindName(props.kind)}</Badge>;
}
