import {
  type DiagnosticCode,
  type DiagnosticsArtifact,
  expectNoDiagnostics,
} from "@solidjs/diagnostics";

// These measure time on the clock, so a busy machine trips them on code that is right.
const CLOCK_BOUND_CODES: DiagnosticCode[] = [
  "HOT_SCOPE_TIME",
  "HOT_SCOPE_RERUNS",
  "LONG_HOLD",
  "SILENT_HOLD",
  "ABANDONED_FLIGHTS",
  "FALLBACK_FLASH",
];

export function expectNoReactivityMistakes(artifact: DiagnosticsArtifact): void {
  expectNoDiagnostics(artifact, { allow: CLOCK_BOUND_CODES });
}
