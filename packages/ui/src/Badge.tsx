import type { JSX } from "solid-js";

const tones = {
  trace: "bg-trace/15 text-trace",
  muted: "bg-muted/12 text-muted",
  info: "bg-info/12 text-info",
  success: "bg-success/14 text-success",
  warn: "bg-warn/15 text-warn",
  error: "bg-error/12 text-error",
  fatal: "bg-fatal/15 text-fatal",
  database: "bg-database/14 text-database",
  placeholder: "bg-placeholder/14 text-placeholder",
} as const;

export type Tone = keyof typeof tones;

/** A short label on a tint of the color of its `tone`. It keeps the width
 * of its text when it sits in a grid cell, which would stretch it. */
export default function Badge(props: { tone: Tone; title?: string; children: JSX.Element }) {
  return (
    <span
      class={`inline-block shrink-0 justify-self-start rounded px-1 text-2xs leading-4.5 font-semibold whitespace-nowrap ${tones[props.tone]}`}
      title={props.title}
    >
      {props.children}
    </span>
  );
}
