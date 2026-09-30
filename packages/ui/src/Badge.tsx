import type { JSX } from "solid-js";

const TONE_CLASSES: Record<BadgeTone, string> = {
  trace: "bg-trace/15 text-trace",
  muted: "bg-muted/12 text-muted",
  info: "bg-info/12 text-info",
  success: "bg-success/14 text-success",
  warn: "bg-warn/15 text-warn",
  error: "bg-error/12 text-error",
  fatal: "bg-fatal/15 text-fatal",
  database: "bg-database/14 text-database",
  placeholder: "bg-placeholder/14 text-placeholder",
};

export type BadgeTone =
  | "trace"
  | "muted"
  | "info"
  | "success"
  | "warn"
  | "error"
  | "fatal"
  | "database"
  | "placeholder";

interface BadgeProps {
  readonly tone: BadgeTone;
  readonly children: JSX.Element;
}

export default function Badge(props: BadgeProps) {
  return (
    <span
      // Keeps the width of its text in a grid cell, which would stretch it.
      class={`inline-block shrink-0 justify-self-start rounded px-1 text-2xs leading-4.5 font-semibold whitespace-nowrap ${TONE_CLASSES[props.tone]}`}
    >
      {props.children}
    </span>
  );
}
