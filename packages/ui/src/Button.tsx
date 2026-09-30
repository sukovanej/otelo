import { type JSX, splitProps } from "solid-js";

import { control, cx, plain, type Size, sizes } from "./classes";

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary: "border-accent bg-accent font-semibold text-on-accent enabled:hover:brightness-110",
  default: `${plain} enabled:hover:bg-hover`,
  ghost:
    "border-transparent bg-transparent text-muted enabled:hover:bg-hover enabled:hover:text-ink",
};

type ButtonVariant = "primary" | "default" | "ghost";

interface ButtonProps extends JSX.ButtonHTMLAttributes<HTMLButtonElement> {
  readonly variant?: ButtonVariant;
  readonly size?: Size | undefined;
}

export default function Button(props: ButtonProps) {
  const [own, rest] = splitProps(props, ["variant", "size", "class", "type"]);
  return (
    <button
      type={own.type ?? "button"}
      class={cx(
        control,
        sizes[own.size ?? "md"],
        VARIANT_CLASSES[own.variant ?? "default"],
        "cursor-pointer disabled:cursor-default disabled:opacity-50",
        own.class,
      )}
      {...rest}
    />
  );
}
