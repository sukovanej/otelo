/** A value a table sorts by. `null` sorts last in either direction. */
export type SortValue = number | string | null;

/** `rows` sorted by the value `by` reads, the largest first when
 * `descending`. Rows with equal values keep their order. */
export function sortRows<R>(
  rows: readonly R[],
  by: (row: R) => SortValue,
  descending: boolean,
): R[] {
  const sign = descending ? -1 : 1;
  return rows
    .map((row, i) => ({ row, i, value: by(row) }))
    .toSorted((a, b) => {
      if (a.value === null || b.value === null) {
        return a.value === b.value ? a.i - b.i : a.value === null ? 1 : -1;
      }
      const order =
        typeof a.value === "string" || typeof b.value === "string"
          ? String(a.value).localeCompare(String(b.value))
          : a.value - b.value;
      return order === 0 ? a.i - b.i : order * sign;
    })
    .map(({ row }) => row);
}
