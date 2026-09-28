import { type JSX, splitProps } from "solid-js";
import { control, cx, plain, type Size, sizes } from "./classes";

const variants = {
  primary: "border-accent bg-accent font-semibold text-on-accent enabled:hover:brightness-110",
  default: `${plain} enabled:hover:bg-hover`,
} as const;

/** A button. `primary` is the one action a view leads with. `class` places
 * it; the size and the variant style it. */
export default function Button(
  props: JSX.ButtonHTMLAttributes<HTMLButtonElement> & {
    variant?: keyof typeof variants;
    size?: Size;
  },
) {
  const [own, rest] = splitProps(props, ["variant", "size", "class", "type"]);
  return (
    <button
      type={own.type ?? "button"}
      class={cx(
        control,
        sizes[own.size ?? "md"],
        variants[own.variant ?? "default"],
        "cursor-pointer disabled:cursor-default disabled:opacity-50",
        own.class,
      )}
      {...rest}
    />
  );
}
