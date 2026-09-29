// How the logs pages name the severity of a line.

const LEVELS = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR", "FATAL"];

/** The name of an OpenTelemetry severity number: TRACE for 1 to 4, DEBUG for
 * 5 to 8, and so on up to FATAL for 21 to 24, or UNSPECIFIED outside them. */
export const levelName = (severity: number) =>
  (severity >= 1 && severity <= 24 && LEVELS[Math.floor((severity - 1) / 4)]) || "UNSPECIFIED";
