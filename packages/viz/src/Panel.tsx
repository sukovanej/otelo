import { type JSX, Show } from "solid-js";

interface PanelProps {
  readonly title?: string | undefined;
  readonly description?: JSX.Element | undefined;
  readonly actions?: JSX.Element | undefined;
  readonly flush?: boolean | undefined;
  readonly children: JSX.Element;
}

export default function Panel(props: PanelProps) {
  return (
    <section
      class="flex min-w-0 flex-col overflow-clip rounded-lg border border-line bg-panel"
      // What sits on the panel and has to cover what scrolls under it, such
      // as the header of a table, takes the color of the panel.
      style={{ "--viz-surface": "var(--color-panel)" }}
    >
      <Show when={props.title || props.actions}>
        <header class="flex min-h-10 flex-wrap items-center gap-x-3 gap-y-1 px-4 pt-3 pb-1">
          <div class="min-w-0 flex-1 basis-48">
            <Show when={props.title}>
              <h2 class="m-0 truncate text-sm font-semibold">{props.title}</h2>
            </Show>
            <Show when={props.description}>
              <div class="truncate text-xs text-muted">{props.description}</div>
            </Show>
          </div>
          <Show when={props.actions}>
            <div class="flex max-w-full min-w-0 items-center gap-3 text-xs">{props.actions}</div>
          </Show>
        </header>
      </Show>
      <div
        // Hides the border under the last row of a table behind the panel's own.
        class={props.flush ? "-mb-px min-w-0" : "min-w-0 px-4 pb-3"}
      >
        {props.children}
      </div>
    </section>
  );
}
