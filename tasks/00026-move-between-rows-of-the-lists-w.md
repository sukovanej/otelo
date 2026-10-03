---
status: todo
created: 2026-10-03T20:22:27Z
tags:
- feature
---
# Move between rows of the lists with the arrow keys

In the web UI, ArrowUp and ArrowDown move the selection one row up or down in each list: traces, spans, the spans of a trace, log lines, log groups, metric names, and services. Enter opens the selected row, and the open panel (a span or a log line) follows the selection. Escape closes the panel and keeps the selection.

The keys do nothing while focus is in an input, a select, or a textarea, the same as `/` and Escape in `usePageKeys` (`packages/app/src/list.ts`). Ctrl-j and Ctrl-k move the selection too, as `toListStep` in `packages/ui/src/keys.ts` already does for the menus. The view scrolls to keep the selected row on screen.
