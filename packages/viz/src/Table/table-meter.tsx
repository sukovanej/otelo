import type { JSX } from "solid-js";

interface TableMeterProps {
  readonly share: number;
  readonly children: JSX.Element;
}

export default function TableMeter(props: TableMeterProps) {
  return (
    <span class="grid w-full min-w-0 grid-cols-[minmax(2ch,1fr)_auto] items-center gap-2">
      <span class="h-1.5 overflow-hidden rounded-full bg-hover" aria-hidden="true">
        <span
          class="block h-full min-w-px rounded-full bg-accent/45 dark:bg-accent/75"
          style={{ width: `${Math.max(0, Math.min(1, props.share)) * 100}%` }}
        />
      </span>
      <span class="text-right whitespace-nowrap">{props.children}</span>
    </span>
  );
}
