// Tailwind finds classes by scanning the source, so each one is written out whole.

// Two classes that set one property do not override each other by their order
// in `class`, so the colors of a control are apart, in `plain`.
export const control = "rounded-md border";

export const plain = "border-line bg-surface";

export const sizes: Record<Size, string> = {
  sm: "h-6 px-1.5 text-2xs",
  md: "h-8 px-2.5",
  lg: "h-10.5 px-3.5",
};

export const textInput = "focus:border-line-focus focus:outline-none";

export const popover =
  "absolute top-[calc(100%+4px)] z-10 rounded-lg border border-line bg-surface shadow-popup";

export const popup = `${popover} m-0 max-h-80 list-none overflow-y-auto p-1`;

export const option = "flex cursor-pointer rounded-[5px] px-2 py-1";

export const activeOption = "bg-active";

export type Size = "sm" | "md" | "lg";

export const cx = (...classes: (string | false | undefined)[]) => classes.filter(Boolean).join(" ");
