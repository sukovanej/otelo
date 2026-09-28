import { CloseIcon } from "@siner/icons";
import { Button } from "@siner/ui";
import type { JSX } from "solid-js";

/** A panel beside a list that shows one of its rows in full: a bar of
 * `header` and Close over the scrolling `children`. */
export default function Panel(props: {
  label: string;
  header: JSX.Element;
  onClose: () => void;
  children: JSX.Element;
}) {
  return (
    <aside
      aria-label={props.label}
      class="relative z-10 flex w-[clamp(24rem,42%,44rem)] shrink-0 flex-col border-l border-line bg-surface shadow-(--floating) motion-safe:animate-panel-in"
    >
      <div class="flex shrink-0 items-center gap-3 border-b border-line bg-subtle px-4 py-2.5">
        {props.header}
        <span class="flex-1" />
        <CloseButton onClose={props.onClose} />
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto px-4 py-3">{props.children}</div>
    </aside>
  );
}

/** A small button with a cross that closes a panel or a modal. */
export function CloseButton(props: { onClose: () => void }) {
  return (
    <Button
      size="sm"
      variant="ghost"
      aria-label="Close"
      title="Close (Esc)"
      onClick={() => props.onClose()}
    >
      <CloseIcon size={13} />
    </Button>
  );
}
