import type { WidgetLayout } from "@otelo/api";

export const WIDE_GRID_COLUMNS = 12;

export const MIN_WIDGET_ROWS = 2;

export const MAX_WIDGET_ROWS = 16;

const MEDIUM_GRID_FROM_PX = 768;
const WIDE_GRID_FROM_PX = 1024;

export type GridColumnCount = 12 | 6 | 2;

export function pickGridColumnCount(gridWidthPx: number): GridColumnCount {
  if (gridWidthPx >= WIDE_GRID_FROM_PX) return 12;
  return gridWidthPx >= MEDIUM_GRID_FROM_PX ? 6 : 2;
}

export function findBottomRow(layouts: ReadonlyArray<WidgetLayout>): number {
  return Math.max(0, ...layouts.map((layout) => layout.row + layout.height));
}

export function resizeLayout(layout: WidgetLayout, width: number, height: number): WidgetLayout {
  const clampedWidth = Math.min(WIDE_GRID_COLUMNS, Math.max(1, width));
  return {
    column: Math.min(layout.column, WIDE_GRID_COLUMNS - clampedWidth),
    row: layout.row,
    width: clampedWidth,
    height: Math.min(MAX_WIDGET_ROWS, Math.max(MIN_WIDGET_ROWS, height)),
  };
}

export function placeBelow(
  layouts: ReadonlyArray<WidgetLayout>,
  layout: WidgetLayout,
): WidgetLayout {
  return { ...layout, column: 0, row: findBottomRow(layouts) };
}

export function placeBeside(layout: WidgetLayout): WidgetLayout {
  return layout.column + 2 * layout.width <= WIDE_GRID_COLUMNS
    ? { ...layout, column: layout.column + layout.width }
    : { ...layout, row: layout.row + layout.height };
}

// The pinned widget keeps its area. A widget it lands on moves above it when
// the area there is free, so two widgets swap, and otherwise below it, as does
// every widget a moved one lands on. Each widget then floats up as far as the
// widgets above it let it, so no move leaves a gap behind.
export function settleLayouts(
  layouts: ReadonlyArray<WidgetLayout>,
  pinnedIndex: number | undefined,
): WidgetLayout[] {
  const placedByIndex = new Map<number, WidgetLayout>();
  const pinned = pinnedIndex === undefined ? undefined : layouts[pinnedIndex];
  if (pinnedIndex !== undefined && pinned) placedByIndex.set(pinnedIndex, pinned);
  for (const index of listInReadingOrder(layouts)) {
    if (index === pinnedIndex) continue;
    const layout = layouts[index];
    if (!layout) continue;
    const above =
      pinned && doLayoutsOverlap(pinned, layout)
        ? { ...layout, row: Math.max(0, pinned.row - layout.height) }
        : undefined;
    placedByIndex.set(
      index,
      above && !findBlocker(placedByIndex, above)
        ? above
        : moveBelowBlockers(placedByIndex, layout),
    );
  }
  return floatLayoutsUp(layouts.map((layout, index) => placedByIndex.get(index) ?? layout));
}

// A narrower grid scales each widget down and flows the widgets in the order
// a wide screen reads them: each takes the first free place that does not come
// before the widget read before it, so neighbours stay together and no hole is
// left that a later widget fits.
export function arrangeLayouts(
  layouts: ReadonlyArray<WidgetLayout>,
  columns: GridColumnCount,
): WidgetLayout[] {
  if (columns === WIDE_GRID_COLUMNS) return [...layouts];
  const placedByIndex = new Map<number, WidgetLayout>();
  let previous: WidgetLayout | undefined;
  for (const index of listInReadingOrder(layouts)) {
    const layout = layouts[index];
    if (!layout) continue;
    const placed = findFirstFreeArea(
      placedByIndex,
      scaleLayout(layout, columns),
      columns,
      previous,
    );
    placedByIndex.set(index, placed);
    previous = placed;
  }
  return layouts.map((layout, index) => placedByIndex.get(index) ?? layout);
}

function floatLayoutsUp(layouts: ReadonlyArray<WidgetLayout>): WidgetLayout[] {
  const placedByIndex = new Map<number, WidgetLayout>();
  for (const index of listInReadingOrder(layouts)) {
    const layout = layouts[index];
    if (!layout) continue;
    let row = layout.row;
    while (row > 0 && !findBlocker(placedByIndex, { ...layout, row: row - 1 })) row -= 1;
    placedByIndex.set(index, { ...layout, row });
  }
  return layouts.map((layout, index) => placedByIndex.get(index) ?? layout);
}

function findFirstFreeArea(
  placedByIndex: ReadonlyMap<number, WidgetLayout>,
  scaled: WidgetLayout,
  columns: GridColumnCount,
  previous: WidgetLayout | undefined,
): WidgetLayout {
  for (let row = previous?.row ?? 0; ; row++) {
    const firstColumn = previous && row === previous.row ? previous.column + previous.width : 0;
    for (let column = firstColumn; column + scaled.width <= columns; column++) {
      const candidate = { ...scaled, column, row };
      if (!findBlocker(placedByIndex, candidate)) return candidate;
    }
  }
}

function moveBelowBlockers(
  placedByIndex: ReadonlyMap<number, WidgetLayout>,
  layout: WidgetLayout,
): WidgetLayout {
  let row = layout.row;
  for (
    let blocker = findBlocker(placedByIndex, layout);
    blocker;
    blocker = findBlocker(placedByIndex, { ...layout, row })
  ) {
    row = blocker.row + blocker.height;
  }
  return { ...layout, row };
}

function findBlocker(
  placedByIndex: ReadonlyMap<number, WidgetLayout>,
  layout: WidgetLayout,
): WidgetLayout | undefined {
  for (const placed of placedByIndex.values()) {
    if (doLayoutsOverlap(placed, layout)) return placed;
  }
  return undefined;
}

function listInReadingOrder(layouts: ReadonlyArray<WidgetLayout>): number[] {
  return layouts
    .map((_, index) => index)
    .toSorted((a, b) => {
      const layoutA = layouts[a];
      const layoutB = layouts[b];
      if (!layoutA || !layoutB) return 0;
      return layoutA.row - layoutB.row || layoutA.column - layoutB.column || a - b;
    });
}

function scaleLayout(layout: WidgetLayout, columns: GridColumnCount): WidgetLayout {
  if (columns === WIDE_GRID_COLUMNS) return layout;
  const width = columns === 2 ? (layout.width <= 3 ? 1 : 2) : Math.ceil(layout.width / 2);
  return { column: 0, row: layout.row, width, height: layout.height };
}

function doLayoutsOverlap(a: WidgetLayout, b: WidgetLayout): boolean {
  return (
    a.column < b.column + b.width &&
    b.column < a.column + a.width &&
    a.row < b.row + b.height &&
    b.row < a.row + a.height
  );
}
