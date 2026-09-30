import { children, type JSX, Show } from "solid-js";

import { Checkbox } from "@otelo/ui";

import type { Fetched } from "./fetch";
import { LIVE_MS } from "./list";
import { formatTime } from "./time";

interface PageBarProps {
  readonly top?: JSX.Element;
  readonly fetched: Pick<Fetched<unknown>, "loading" | "updated">;
  readonly end?: JSX.Element;
  readonly children?: JSX.Element;
}

/**
 * The top of every page under the top bar, which stays while the page
 * scrolls. With `top`, it has two rows: `top` holds what the page asks for
 * or shows, such as the query and the range, or the name of a trace, and the
 * second row the views and what the answer holds (`children`). Without it,
 * `children` share one row with the rest. The row of `children` ends with
 * whether a request runs, when the last answer arrived, and `end`, such as
 * the range and live mode.
 */
export default function PageBar(props: PageBarProps) {
  const top = children(() => props.top);
  return (
    <div
      class="relative z-20 shrink-0 bg-surface px-4 shadow-(--raised)"
      classList={{ "pt-3.5": !!top() }}
    >
      {top()}
      <div
        class="flex min-h-7 items-center gap-3 text-muted"
        classList={{ "pt-3 pb-2.5": !!top(), "py-2.5": !top() }}
      >
        {props.children}
        <span class="flex-1" />
        <Show when={props.fetched.loading()}>
          <span aria-live="polite">Loading…</span>
        </Show>
        <Show when={props.fetched.updated()}>
          {(updated) => <span>Updated {formatTime(updated())}</span>}
        </Show>
        {props.end}
      </div>
    </div>
  );
}

/** The switch of live mode, which needs a range that ends now. */
export function LiveToggle(props: {
  live: boolean;
  until: string;
  onChange: (live: boolean) => void;
}) {
  return (
    <Checkbox
      checked={props.live}
      disabled={props.until !== ""}
      onChange={(checked) => props.onChange(checked)}
      title={
        props.until === "" ? `Reload every ${LIVE_MS / 1000} s` : "Live needs a range that ends now"
      }
    >
      Live
    </Checkbox>
  );
}
