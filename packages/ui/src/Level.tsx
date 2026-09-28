const tones: Record<string, string> = {
  TRACE: "bg-trace/15 text-trace",
  DEBUG: "bg-debug/12 text-debug",
  INFO: "bg-info/12 text-info",
  WARN: "bg-warn/15 text-warn",
  ERROR: "bg-error/12 text-error",
  FATAL: "bg-fatal/15 text-fatal",
};

/** The name of a log severity on a tint of its color: TRACE, DEBUG, INFO,
 * WARN, ERROR, FATAL, or UNSPECIFIED. It keeps the width of its name when it
 * sits in a grid cell, which would stretch it. */
export default function Level(props: { level: string }) {
  return (
    <span
      class={`inline-block justify-self-start rounded px-1 text-2xs leading-4 font-semibold ${tones[props.level] ?? "bg-muted/12 text-muted"}`}
    >
      {props.level}
    </span>
  );
}
