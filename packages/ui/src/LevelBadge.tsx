import Badge, { type BadgeTone } from "./Badge";

const LEVEL_TONES: Record<LevelName, BadgeTone> = {
  TRACE: "trace",
  DEBUG: "muted",
  INFO: "info",
  WARN: "warn",
  ERROR: "error",
  FATAL: "fatal",
  UNSPECIFIED: "muted",
};

export type LevelName = "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR" | "FATAL" | "UNSPECIFIED";

interface LevelBadgeProps {
  readonly level: LevelName;
}

export default function LevelBadge(props: LevelBadgeProps) {
  return <Badge tone={LEVEL_TONES[props.level]}>{props.level}</Badge>;
}
