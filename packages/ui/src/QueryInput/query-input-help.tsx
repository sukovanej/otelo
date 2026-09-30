import type { JSX } from "solid-js";

import { cx, popover } from "../classes";

const HELP_WIDTH_PX = 360;

interface QueryInputHelpProps {
  readonly left: number;
  readonly children: JSX.Element;
}

export default function QueryInputHelp(props: QueryInputHelpProps) {
  return (
    <div
      role="tooltip"
      class={cx(popover, "pointer-events-none max-w-full p-3")}
      // Moved left by its border and padding, so its text lines up with the
      // token.
      style={{
        width: `${HELP_WIDTH_PX}px`,
        left: `max(0px, min(${props.left}px - 13px, 100% - ${HELP_WIDTH_PX}px))`,
      }}
    >
      {props.children}
    </div>
  );
}
