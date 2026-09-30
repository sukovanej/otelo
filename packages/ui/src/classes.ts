// Class lists the components share. Tailwind finds classes by scanning the
// source, so each one is written out whole.

/** The frame of a control: a button, a text input, a select. Its colors are
 * apart in `plain`, since two classes that set the same property do not
 * override each other by their order in `class`. */
export const control = "rounded-md border";

/** The colors of a control that does not stand out. */
export const plain = "border-line bg-surface";

/** The height, padding, and text of a control of each size. */
export const sizes = {
  sm: "h-6 px-1.5 text-2xs",
  md: "h-8 px-2.5",
  lg: "h-10.5 px-3.5",
} as const;

export type Size = keyof typeof sizes;

/** A text input shows its focus with a slightly darker border, not an
 * outline. */
export const textInput = "focus:border-line-focus focus:outline-none";

/** What opens under a control, which its `relative` parent places. */
export const popover =
  "absolute top-[calc(100%+4px)] z-10 rounded-lg border border-line bg-surface shadow-popup";

/** A list that opens under a control. */
export const popup = `${popover} m-0 max-h-80 list-none overflow-y-auto p-1`;

/** An option of a popup list, and the one the keys or the pointer are on.
 * The list lays out its content with `items-*` and `gap-*`. */
export const option = "flex cursor-pointer rounded-[5px] px-2 py-1";
export const activeOption = "bg-active";

/** Joins class lists, leaving out the empty ones. */
export const cx = (...classes: (string | false | undefined)[]) => classes.filter(Boolean).join(" ");
