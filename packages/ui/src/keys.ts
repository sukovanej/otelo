// Keys that move the highlight through a list: the arrows, and Ctrl+J and
// Ctrl+K as j and k move in Vim.

type Key = Pick<KeyboardEvent, "key" | "ctrlKey" | "altKey" | "metaKey" | "shiftKey">;

const ctrl = (e: Key, letter: string) =>
  e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey && e.key.toLowerCase() === letter;

/** 1 for a key that moves down, -1 for one that moves up, else 0. */
export function listStep(e: Key): number {
  if (e.key === "ArrowDown" || ctrl(e, "j")) return 1;
  if (e.key === "ArrowUp" || ctrl(e, "k")) return -1;
  return 0;
}

/**
 * The index `step` items away from `index` in a list of `count`, wrapping at
 * the ends. With `none`, -1 is a stop before the first item, which stands for
 * no highlight.
 */
export function move(index: number, step: number, count: number, none = false): number {
  const low = none ? -1 : 0;
  const size = count - low;
  if (size <= 0) return low;
  return ((((index - low + step) % size) + size) % size) + low;
}
