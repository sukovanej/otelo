// Class lists the views share.

/** The panel under an open line or template. */
export const detail = "bg-subtle px-3 pt-2 pb-3 font-mono";

/** A log body in full. */
export const body =
  "mb-1.5 rounded-md border border-line bg-surface px-2.5 py-2 font-mono whitespace-pre-wrap wrap-anywhere";

/** The times under a body. */
export const times = "mb-2 text-xs text-muted";

/** The name of a section of a panel. */
export const heading =
  "mt-2.5 mb-1 font-sans text-2xs font-semibold tracking-[0.04em] text-muted uppercase";

/** The part of a page under its bar, which scrolls. The gap under the bar
 * is a margin of the first part in it, not a padding: a sticky header, such
 * as the one of a table, sticks inside the padding of what scrolls, so a
 * padding would leave rows showing above it. */
export const pageContent = "overflow-y-auto px-4 pb-8 [&>:first-child]:mt-5";

/** A link in the text of a view. */
export const link = "text-accent hover:underline";
