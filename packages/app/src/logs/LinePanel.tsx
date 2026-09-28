import { Button, Level } from "@siner/ui";
import type { LogLine } from "../api";
import { formatTime, parseTime } from "../time";
import Fields from "./Fields";

/** One log line in full, in a panel beside the list. */
export default function LinePanel(props: {
  line: LogLine;
  onFilter: (term: string) => void;
  onClose: () => void;
}) {
  return (
    <aside
      aria-label="Log line"
      class="relative z-10 flex w-[clamp(24rem,42%,44rem)] shrink-0 flex-col border-l border-line bg-surface shadow-(--floating) motion-safe:animate-panel-in"
    >
      <div class="flex shrink-0 items-center gap-3 border-b border-line bg-subtle px-4 py-2.5">
        <Level level={props.line.level} />
        <span class="font-mono text-sm">{formatTime(parseTime(props.line.time))}</span>
        <span class="truncate font-mono text-sm text-muted">{props.line.service}</span>
        <span class="flex-1" />
        <Button size="sm" aria-label="Close" title="Close (Esc)" onClick={() => props.onClose()}>
          <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
            <path
              d="M3 3l6 6M9 3l-6 6"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
            />
          </svg>
        </Button>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        <Fields line={props.line} onFilter={props.onFilter} />
      </div>
    </aside>
  );
}
