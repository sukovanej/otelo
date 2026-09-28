import type { JSX } from "solid-js";

/** A value in a cell, right-aligned, after a bar of its `share` of the
 * largest value of the column, from 0 to 1. The bar keeps to its own track,
 * so it never runs under the value. */
export default function Meter(props: { share: number; children: JSX.Element }) {
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
