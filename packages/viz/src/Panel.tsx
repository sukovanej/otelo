import { type JSX, Show } from "solid-js";

/**
 * The frame of one part of a page or a dashboard: a title, an optional line
 * under it, actions on the right, and the chart, table, or numbers inside.
 * `flush` lets a table run to the edges of the frame, which clips it to its
 * rounded corners and hides the border under its last row behind its own.
 * The panel is a step off the color of the page, so it stands apart from it.
 */
export default function Panel(props: {
  title?: string;
  description?: JSX.Element;
  actions?: JSX.Element;
  flush?: boolean;
  class?: string;
  children: JSX.Element;
}) {
  return (
    <section
      class={`flex min-w-0 flex-col overflow-clip rounded-lg border border-line bg-panel ${props.class ?? ""}`}
      // What sits on the panel and has to cover what scrolls under it, such
      // as the header of a table, takes the color of the panel.
      style={{ "--viz-surface": "var(--color-panel)" }}
    >
      <Show when={props.title || props.actions}>
        <header class="flex min-h-10 items-center gap-3 px-4 pt-3 pb-1">
          <div class="min-w-0 flex-1">
            <Show when={props.title}>
              <h2 class="m-0 truncate text-sm font-semibold">{props.title}</h2>
            </Show>
            <Show when={props.description}>
              <div class="truncate text-xs text-muted">{props.description}</div>
            </Show>
          </div>
          <Show when={props.actions}>
            <div class="flex shrink-0 items-center gap-3 text-xs">{props.actions}</div>
          </Show>
        </header>
      </Show>
      <div class={props.flush ? "-mb-px min-w-0" : "min-w-0 px-4 pb-3"}>{props.children}</div>
    </section>
  );
}
