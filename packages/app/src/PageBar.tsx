import { children, type JSX, Show } from "solid-js";

import type { FetchState } from "./fetch";
import { formatTime } from "./time";

interface PageBarProps {
  readonly top?: JSX.Element;
  readonly fetched: Pick<FetchState<unknown>, "loading" | "updatedAt">;
  readonly end: JSX.Element;
  readonly children: JSX.Element;
}

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
        <Show when={props.fetched.updatedAt()}>
          {(updatedAt) => <span>Updated {formatTime(updatedAt())}</span>}
        </Show>
        {props.end}
      </div>
    </div>
  );
}
