import type { JSX } from "solid-js";

/** A number in a grid cell, right-aligned, after a bar of its `share` of the
 * largest number of the column, from 0 to 1. The bar keeps to its own track,
 * so it never runs under the number. */
export default function Measure(props: { share: number; children: JSX.Element }) {
  return (
    <span class="grid grid-cols-[minmax(2ch,1fr)_auto] items-center gap-2">
      <span class="h-1.5 overflow-hidden rounded-full bg-hover" aria-hidden="true">
        <span
          class="block h-full min-w-px rounded-full bg-accent/45 dark:bg-accent/75"
          style={{ width: `${100 * props.share}%` }}
        />
      </span>
      <span class="text-right whitespace-nowrap">{props.children}</span>
    </span>
  );
}
