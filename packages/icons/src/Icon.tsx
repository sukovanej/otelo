import type { JSX } from "solid-js";

export interface IconProps {
  /** The width and the height, in pixels. 16 when missing. */
  size?: number | undefined;
  /** Classes for the `<svg>`, such as a color for `currentColor`. */
  class?: string | undefined;
  /** What the icon means, for a reader that cannot see it. An icon without
   * a title is decoration, and hidden from them. To show it on hover, put the
   * icon in the `Tooltip` of `@otelo/ui`; the icon draws no tooltip itself. */
  title?: string | undefined;
}

/** The frame of every icon: a square `<svg>` on a 16 by 16 grid. In a row
 * that lines up on the baseline of its text, it centers itself instead, so the
 * row takes its baseline from the text. */
export default function Icon(props: IconProps & { children: JSX.Element }) {
  return (
    <svg
      width={props.size ?? 16}
      height={props.size ?? 16}
      viewBox="0 0 16 16"
      class={`shrink-0 self-center ${props.class ?? ""}`}
      role={props.title ? "img" : undefined}
      aria-label={props.title}
      aria-hidden={props.title ? undefined : "true"}
    >
      {props.children}
    </svg>
  );
}
