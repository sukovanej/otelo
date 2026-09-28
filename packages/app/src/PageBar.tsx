import { type JSX, Show } from "solid-js";

import { Checkbox } from "@siner/ui";

import type { Fetched } from "./fetch";
import { LIVE_MS } from "./list";
import { formatTime } from "./time";

/**
 * The top of every page under the top bar, which stays while the page
 * scrolls. With `top`, it has two rows: `top` holds what the page asks for
 * or shows, such as the query and the range, or the name of a trace, and the
 * second row the views and what the answer holds (`children`). Without it,
 * `children` share one row with the rest. The row of `children` ends with
 * whether a request runs, when the last answer arrived, and `end`, such as
 * the range and live mode.
 */
export default function PageBar(props: {
  top?: JSX.Element;
  fetched: Pick<Fetched<unknown>, "loading" | "updated">;
  end?: JSX.Element;
  children?: JSX.Element;
}) {
  return (
    <div
      class="relative z-20 shrink-0 bg-surface px-4 shadow-(--raised)"
      classList={{ "pt-3.5": !!props.top }}
    >
      {props.top}
      <div
        class="flex min-h-7 items-center gap-3 text-muted"
        classList={{ "pt-3 pb-2.5": !!props.top, "py-2.5": !props.top }}
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
