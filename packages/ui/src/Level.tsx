import Badge, { type Tone } from "./Badge";

const LEVEL_TONES: Record<string, Tone> = {
  TRACE: "trace",
  DEBUG: "muted",
  INFO: "info",
  WARN: "warn",
  ERROR: "error",
  FATAL: "fatal",
};

interface LevelProps {
  readonly level: string;
}

export default function Level(props: LevelProps) {
  return <Badge tone={LEVEL_TONES[props.level] ?? "muted"}>{props.level}</Badge>;
}
