import type { JSX } from "solid-js";

const tones = {
  error: "whitespace-pre-wrap bg-error-soft font-mono text-error",
  hint: "flex flex-wrap items-center gap-2 bg-hint",
} as const;

/** A message above the content: an `error` that stopped it, or a `hint`
 * about it. */
export default function Callout(props: { tone: keyof typeof tones; children: JSX.Element }) {
  return (
    <div
      class={`mb-2 rounded-md px-3 py-2 ${tones[props.tone]}`}
      role={props.tone === "error" ? "alert" : undefined}
    >
      {props.children}
    </div>
  );
}
