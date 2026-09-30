import type { JSX } from "solid-js";

const TONE_CLASSES: Record<CalloutTone, string> = {
  error: "whitespace-pre-wrap bg-error-soft font-mono text-error",
  hint: "flex flex-wrap items-center gap-2 bg-hint",
};

type CalloutTone = "error" | "hint";

interface CalloutProps {
  readonly tone: CalloutTone;
  readonly children: JSX.Element;
}

export default function Callout(props: CalloutProps) {
  return (
    <div
      class={`mb-2 rounded-md px-3 py-2 ${TONE_CLASSES[props.tone]}`}
      role={props.tone === "error" ? "alert" : undefined}
    >
      {props.children}
    </div>
  );
}
