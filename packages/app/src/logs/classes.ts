// Class lists the log views share.

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

/** The columns of the lines and of the templates, for their rows and
 * headers alike. */
export const lineColumns = "grid-cols-[var(--time-width)_6.5ch_minmax(6ch,16ch)_1fr]";
export const groupColumns = "grid-cols-[10ch_6.5ch_1fr_minmax(8ch,20ch)_7ch]";

/** The names of the columns, which stay at the top while the rows scroll.
 * The header keeps the font of the rows, because the columns are sized in
 * `ch` of its font, and sets the labels smaller. */
export const header =
  "sticky top-0 z-10 grid items-baseline gap-3 border-b border-line bg-surface px-3 py-2 font-mono text-sm text-muted *:font-sans *:text-2xs *:font-semibold *:tracking-[0.04em] *:uppercase";

/** A row that opens its panel, closed and open. */
export const row = "grid cursor-pointer items-baseline gap-3 px-3";
export const closedRow = "hover:bg-hover";
export const openRow = "bg-active shadow-[inset_3px_0_0_var(--color-accent)]";

/** A closed row of an error or a fatal line, tinted so it stands out. */
export const closedErrorRow = "bg-error/5 hover:bg-error/10";
