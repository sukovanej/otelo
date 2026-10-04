// Inline items wrap in lines, as the tags of a field do; block items stack, as
// the rows of a list do.
export type ReorderAxis = "inline" | "block";

export interface ItemBox {
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
}

export function moveValue<T>(values: ReadonlyArray<T>, value: T, toIndex: number): T[] {
  const others = values.filter((other) => other !== value);
  const index = Math.min(Math.max(0, toIndex), others.length);
  return [...others.slice(0, index), value, ...others.slice(index)];
}

// The place among the other items where an item dragged to the point goes. An
// inline item goes after every item on a line above the point and after the
// items of its line whose middle is left of it; a block item goes after the
// items whose middle is above it.
export function pickDropIndex(
  axis: ReorderAxis,
  otherItems: ReadonlyArray<ItemBox>,
  pointX: number,
  pointY: number,
): number {
  if (axis === "block") {
    return otherItems.filter((item) => (item.top + item.bottom) / 2 < pointY).length;
  }
  return otherItems.filter(
    (item) =>
      item.bottom <= pointY || (item.top <= pointY && (item.left + item.right) / 2 < pointX),
  ).length;
}
