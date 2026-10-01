import type { JSX } from "@solidjs/web";

import CloseButton from "./CloseButton";

interface SidePanelProps {
  readonly label: string;
  readonly header: JSX.Element;
  readonly onClose: () => void;
  readonly children: JSX.Element;
}

export default function SidePanel(props: SidePanelProps) {
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
