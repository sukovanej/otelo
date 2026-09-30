import type { JSX } from "solid-js";

const DEFAULT_SIZE_PX = 16;

export interface IconProps {
  readonly size?: number | undefined;
  readonly class?: string | undefined;
  readonly title?: string | undefined;
}

interface IconFrameProps extends IconProps {
  readonly children: JSX.Element;
}

export default function Icon(props: IconFrameProps) {
  return (
    <svg
      width={props.size ?? DEFAULT_SIZE_PX}
      height={props.size ?? DEFAULT_SIZE_PX}
      viewBox="0 0 16 16"
      // Centers itself in a row that lines up on the baseline of its text, so
      // the row takes its baseline from the text.
      class={`shrink-0 self-center ${props.class ?? ""}`}
      role={props.title ? "img" : undefined}
      // The icon draws no tooltip for its title, the `Tooltip` of `@otelo/ui` does.
      aria-label={props.title}
      aria-hidden={props.title ? undefined : "true"}
    >
      {props.children}
    </svg>
  );
}
