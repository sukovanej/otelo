const LEVEL_NAMES = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR", "FATAL"];

const SEVERITIES_PER_LEVEL = 4;

const MAX_SEVERITY = LEVEL_NAMES.length * SEVERITIES_PER_LEVEL;

export function toLevelName(severity: number): string {
  return (
    (severity >= 1 &&
      severity <= MAX_SEVERITY &&
      LEVEL_NAMES[Math.floor((severity - 1) / SEVERITIES_PER_LEVEL)]) ||
    "UNSPECIFIED"
  );
}
