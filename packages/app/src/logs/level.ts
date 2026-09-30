import type { LevelName } from "@otelo/ui";

const LEVEL_NAMES: ReadonlyArray<LevelName> = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR", "FATAL"];

const SEVERITIES_PER_LEVEL = 4;

const MAX_SEVERITY = LEVEL_NAMES.length * SEVERITIES_PER_LEVEL;

export function toLevelName(severity: number): LevelName {
  return (
    (severity >= 1 &&
      severity <= MAX_SEVERITY &&
      LEVEL_NAMES[Math.floor((severity - 1) / SEVERITIES_PER_LEVEL)]) ||
    "UNSPECIFIED"
  );
}
