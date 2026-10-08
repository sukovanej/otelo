import type { JSX } from "@solidjs/web";
import { children, Show } from "solid-js";

interface PanelProps {
  readonly title?: string | undefined;
  readonly description?: JSX.Element | undefined;
  readonly actions?: JSX.Element | undefined;
  readonly flush?: boolean | undefined;
  readonly fill?: boolean | undefined;
  readonly singleLineHeader?: boolean | undefined;
  readonly actionsOnHover?: boolean | undefined;
  readonly children: JSX.Element;
}

export default function Panel(props: PanelProps) {
  const description = children(() => props.description);
  const actions = children(() => props.actions);
  return (
    <section
      class={[
        "group/panel flex min-w-0 flex-col overflow-clip rounded-lg border border-line bg-panel",
        { "h-full": props.fill },
      ]}
      // What sits on the panel and has to cover what scrolls under it, such
      // as the header of a table, takes the color of the panel.
      style={{ "--viz-surface": "var(--color-panel)" }}
    >
      <Show when={props.title || actions()}>
        <header
          class={[
            "flex min-h-10 items-center gap-x-3 gap-y-1 border-b border-line bg-subtle px-4 py-2",
            props.singleLineHeader ? "flex-nowrap" : "flex-wrap",
          ]}
        >
          <div class={["min-w-0 flex-1", { "basis-48": !props.singleLineHeader }]}>
            <Show when={props.title}>
              <h2 class="m-0 truncate text-sm font-semibold">{props.title}</h2>
            </Show>
            <Show when={description()}>
              <div class="truncate text-xs text-muted">{description()}</div>
            </Show>
          </div>
          <Show when={actions()}>
            <div
              class={[
                "flex items-center gap-3 text-xs",
                props.singleLineHeader ? "shrink-0" : "max-w-full min-w-0",
                {
                  "transition-opacity not-group-hover/panel:not-focus-within:not-pointer-coarse:-ms-3 not-group-hover/panel:not-focus-within:not-pointer-coarse:max-w-0 not-group-hover/panel:not-focus-within:not-pointer-coarse:overflow-hidden not-group-hover/panel:not-focus-within:not-pointer-coarse:opacity-0":
                    props.actionsOnHover,
                },
              ]}
            >
              {actions()}
            </div>
          </Show>
        </header>
      </Show>
      <div
        // Hides the border under the last row of a table behind the panel's own.
        class={[
          props.flush ? "-mb-px min-w-0" : "min-w-0 px-4 pb-3",
          { "flex min-h-0 flex-1 flex-col": props.fill },
        ]}
      >
        {props.children}
      </div>
    </section>
  );
}
