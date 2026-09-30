export type SortValue = number | string | null;

export function sortRows<R>(
  rows: ReadonlyArray<R>,
  readSortValue: (row: R) => SortValue,
  descending: boolean,
): R[] {
  const sign = descending ? -1 : 1;
  return rows
    .map((row, index) => ({ row, index, value: readSortValue(row) }))
    .toSorted((a, b) => {
      if (a.value === null || b.value === null) {
        return a.value === b.value ? a.index - b.index : a.value === null ? 1 : -1;
      }
      const order =
        typeof a.value === "string" || typeof b.value === "string"
          ? String(a.value).localeCompare(String(b.value))
          : a.value - b.value;
      return order === 0 ? a.index - b.index : order * sign;
    })
    .map(({ row }) => row);
}
