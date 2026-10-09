import { For } from "solid-js";

const BAR_HEIGHTS_PERCENT = [38, 52, 46, 64, 58, 72, 61, 49, 67, 55, 70, 62];

export default function ChartSkeleton() {
  return (
    <div
      role="status"
      aria-label="Loading"
      class="flex min-h-24 flex-1 items-end gap-1.5 pt-3 motion-safe:animate-pulse"
    >
      <For each={BAR_HEIGHTS_PERCENT}>
        {(height) => <span class="flex-1 rounded-t-sm bg-hover" style={{ height: `${height}%` }} />}
      </For>
    </div>
  );
}
