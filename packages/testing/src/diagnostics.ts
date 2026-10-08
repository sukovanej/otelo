import {
  type DiagnosticCode,
  type DiagnosticsArtifact,
  expectNoDiagnostics,
  expectNoSilentHolds,
} from "@solidjs/diagnostics";

// These measure time on the clock, so a busy machine trips them on code that is right.
// Whether a hold was acknowledged does not depend on the clock, so expectNoSilentHolds
// checks every hold, however short.
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
  expectNoSilentHolds(artifact);
}
