import Badge, { type Tone } from "./Badge";

const tones: Record<string, Tone> = {
  TRACE: "trace",
  DEBUG: "muted",
  INFO: "info",
  WARN: "warn",
  ERROR: "error",
  FATAL: "fatal",
};

/** The name of a log severity on a tint of its color: TRACE, DEBUG, INFO,
 * WARN, ERROR, FATAL, or UNSPECIFIED. */
export default function Level(props: { level: string }) {
  return <Badge tone={tones[props.level] ?? "muted"}>{props.level}</Badge>;
}
