const colors: Record<string, string> = {
  TRACE: "text-trace",
  DEBUG: "text-debug",
  INFO: "text-info",
  WARN: "text-warn",
  ERROR: "text-error",
  FATAL: "text-fatal",
};

/** The name of a log severity, in its color: TRACE, DEBUG, INFO, WARN,
 * ERROR, FATAL, or UNSPECIFIED. */
export default function Level(props: { level: string }) {
  return (
    <span class={`text-2xs font-semibold ${colors[props.level] ?? "text-muted"}`}>
      {props.level}
    </span>
  );
}
