import type { JSX } from "@solidjs/web";
import {
  createEffect,
  createMemo,
  createProjection,
  createSignal,
  For,
  onSettled,
  Show,
} from "solid-js";

import type { Widget, WidgetLayout } from "@otelo/api";
import { ResizeIcon } from "@otelo/icons";

import type { RangeState } from "../../services/range";
import DashboardWidget from "../DashboardWidget";
import {
  arrangeLayouts,
  findBottomRow,
  pickGridColumnCount,
  resizeLayout,
  settleLayouts,
  WIDE_GRID_COLUMNS,
} from "../layout";
import { describeWidgetTitle, GRID_GAP_PX, GRID_ROW_HEIGHT_PX } from "../widget";
import DashboardPageWidgetActions from "./dashboard-page-widget-actions";

const ROW_STEP_PX = GRID_ROW_HEIGHT_PX + GRID_GAP_PX;

// A cell with the corners of a widget, repeated down a column, so no square
// corner of a cell shows past the rounded border of the widget over it.
const GRID_CELL_MASK = `url("data:image/svg+xml,${encodeURIComponent(
  `<svg xmlns="http://www.w3.org/2000/svg"><rect width="100%" height="${GRID_ROW_HEIGHT_PX}" rx="8"/></svg>`,
)}") top / 100% ${ROW_STEP_PX}px repeat-y`;

const ARROW_STEPS: Partial<Record<string, GridStep>> = {
  ArrowLeft: { columns: -1, rows: 0 },
  ArrowRight: { columns: 1, rows: 0 },
  ArrowUp: { columns: 0, rows: -1 },
  ArrowDown: { columns: 0, rows: 1 },
};

const RESIZED_SIDES: Record<ResizeEdge, ResizedSides> = {
  right: { width: true, height: false },
  bottom: { width: false, height: true },
  corner: { width: true, height: true },
};

type Gesture = MoveGesture | ResizeGesture;

type ResizeEdge = "right" | "bottom" | "corner";

interface GestureBase {
  readonly index: number;
  readonly pointerId: number;
  readonly startLayouts: ReadonlyArray<WidgetLayout>;
  readonly layouts: ReadonlyArray<WidgetLayout>;
}

interface MoveGesture extends GestureBase {
  readonly kind: "move";
  readonly grabOffset: PointInGrid;
}

interface ResizeGesture extends GestureBase {
  readonly kind: "resize";
  readonly edge: ResizeEdge;
  readonly startShown: WidgetLayout;
  readonly startPoint: PointInGrid;
}

interface GridStep {
  readonly columns: number;
  readonly rows: number;
}

interface ResizedSides {
  readonly width: boolean;
  readonly height: boolean;
}

interface PointInGrid {
  readonly x: number;
  readonly y: number;
}

interface PointInWindow {
  readonly clientX: number;
  readonly clientY: number;
}

interface DashboardPageGridProps {
  readonly widgets: ReadonlyArray<Widget>;
  readonly range: Pick<RangeState, "since" | "until" | "live" | "setRange">;
  readonly onEdit: (index: number) => void;
  readonly onDuplicate: (index: number) => void;
  readonly onRemove: (index: number) => void;
  readonly onLayoutChange: (layouts: ReadonlyArray<WidgetLayout>) => void;
}

export default function DashboardPageGrid(props: DashboardPageGridProps) {
  let grid!: HTMLDivElement;
  const [gridWidthPx, setGridWidthPx] = createSignal(0);
  const [gesture, setGesture] = createSignal<Gesture>();
  const [pointerInGrid, setPointerInGrid] = createSignal<PointInGrid>({ x: 0, y: 0 });
  let lastPointerInWindow: PointInWindow = { clientX: 0, clientY: 0 };
  onSettled(() => {
    setGridWidthPx(grid.clientWidth);
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry) setGridWidthPx(entry.contentRect.width);
    });
    observer.observe(grid);
    return () => observer.disconnect();
  });

  const columnCount = () => pickGridColumnCount(gridWidthPx());
  // A narrower grid flows the widgets from the wide layout, so only the wide
  // grid moves and resizes them.
  const isArrangeable = () => columnCount() === WIDE_GRID_COLUMNS;
  const columnStepPx = () => (gridWidthPx() + GRID_GAP_PX) / columnCount();
  const savedLayouts = createMemo(() => props.widgets.map((widget) => widget.layout));
  const gestureKindByIndex = createProjection<Partial<Record<number, Gesture["kind"]>>>(() => {
    const current = gesture();
    return current ? { [current.index]: current.kind } : {};
  }, {});
  const shownLayouts = createMemo(() =>
    arrangeLayouts(gesture()?.layouts ?? savedLayouts(), columnCount()),
  );
  const rowCount = createMemo(() => {
    const current = gesture();
    if (current?.kind === "resize") return findBottomRow(shownLayouts()) + 2;
    const movedHeight = current?.kind === "move" ? (shownLayouts()[current.index]?.height ?? 0) : 0;
    return findBottomRow(shownLayouts()) + movedHeight;
  });
  const toPointInGrid = (pointer: PointInWindow): PointInGrid => {
    const rect = grid.getBoundingClientRect();
    return { x: pointer.clientX - rect.left, y: pointer.clientY - rect.top };
  };

  const startMove = (index: number, e: PointerEvent) => {
    const pressed = e.target;
    const item = e.currentTarget;
    if (
      e.button !== 0 ||
      !isArrangeable() ||
      !(pressed instanceof Element) ||
      !(item instanceof Element) ||
      !pressed.closest("header") ||
      pressed.closest("button, a, input, select, textarea, [popover]")
    ) {
      return;
    }
    e.preventDefault();
    item.setPointerCapture(e.pointerId);
    const itemRect = item.getBoundingClientRect();
    lastPointerInWindow = toPointInWindow(e);
    setPointerInGrid(toPointInGrid(e));
    setGesture({
      kind: "move",
      index,
      pointerId: e.pointerId,
      startLayouts: savedLayouts(),
      layouts: savedLayouts(),
      grabOffset: { x: e.clientX - itemRect.left, y: e.clientY - itemRect.top },
    });
  };
  const continueMove = (current: MoveGesture, pointer: PointInWindow) => {
    const pointInGrid = toPointInGrid(pointer);
    const moving = current.startLayouts[current.index];
    if (!moving) return;
    const column = Math.min(
      Math.max(0, Math.round((pointInGrid.x - current.grabOffset.x) / columnStepPx())),
      WIDE_GRID_COLUMNS - moving.width,
    );
    const row = Math.max(0, Math.round((pointInGrid.y - current.grabOffset.y) / ROW_STEP_PX));
    setPointerInGrid(pointInGrid);
    changeGestureLayouts(
      current,
      settleLayouts(
        replaceLayout(current.startLayouts, current.index, { ...moving, column, row }),
        current.index,
      ),
    );
  };
  const changeGestureLayouts = (current: Gesture, layouts: ReadonlyArray<WidgetLayout>) => {
    if (hasLayoutChanged(current.layouts, layouts)) setGesture({ ...current, layouts });
  };

  const startResize = (index: number, edge: ResizeEdge, e: PointerEvent) => {
    const shown = shownLayouts()[index];
    if (e.button !== 0 || !isArrangeable() || !shown || !(e.currentTarget instanceof Element)) {
      return;
    }
    e.preventDefault();
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);
    lastPointerInWindow = toPointInWindow(e);
    setGesture({
      kind: "resize",
      index,
      pointerId: e.pointerId,
      startLayouts: savedLayouts(),
      layouts: savedLayouts(),
      edge,
      startShown: shown,
      startPoint: toPointInGrid(e),
    });
  };
  // The widget takes the next column or row once the pointer passes halfway
  // into it, so its corner is never more than half a cell from the pointer.
  const continueResize = (current: ResizeGesture, pointer: PointInWindow) => {
    const resizing = current.startLayouts[current.index];
    if (!resizing) return;
    const point = toPointInGrid(pointer);
    const sides = RESIZED_SIDES[current.edge];
    const width = sides.width
      ? resizing.width + Math.round((point.x - current.startPoint.x) / columnStepPx())
      : resizing.width;
    const height = sides.height
      ? resizing.height + Math.round((point.y - current.startPoint.y) / ROW_STEP_PX)
      : resizing.height;
    changeGestureLayouts(
      current,
      settleLayouts(
        replaceLayout(current.startLayouts, current.index, resizeLayout(resizing, width, height)),
        current.index,
      ),
    );
  };

  const continueGestureAt = (current: Gesture, pointer: PointInWindow) => {
    lastPointerInWindow = pointer;
    if (current.kind === "move") continueMove(current, pointer);
    else continueResize(current, pointer);
  };
  const continueGesture = (e: PointerEvent) => {
    const current = gesture();
    if (current?.pointerId === e.pointerId) continueGestureAt(current, toPointInWindow(e));
  };
  const followScroll = () => {
    const current = gesture();
    if (current) continueGestureAt(current, lastPointerInWindow);
  };
  createEffect(
    () => gesture() !== undefined,
    (isGestureActive) => {
      if (!isGestureActive) return undefined;
      window.addEventListener("scroll", followScroll, true);
      return () => window.removeEventListener("scroll", followScroll, true);
    },
  );
  const endGesture = (e: PointerEvent) => {
    const current = gesture();
    if (current?.pointerId !== e.pointerId) return;
    setGesture(undefined);
    if (hasLayoutChanged(current.startLayouts, current.layouts)) {
      props.onLayoutChange(current.layouts);
    }
  };
  const changeLayoutWithKeys = (index: number, e: KeyboardEvent) => {
    const layout = savedLayouts()[index];
    const step = ARROW_STEPS[e.key];
    if (!layout || !step) return;
    e.preventDefault();
    const changed = e.shiftKey
      ? {
          ...layout,
          column: Math.min(
            WIDE_GRID_COLUMNS - layout.width,
            Math.max(0, layout.column + step.columns),
          ),
          row: Math.max(0, layout.row + step.rows),
        }
      : resizeLayout(layout, layout.width + step.columns, layout.height + step.rows);
    props.onLayoutChange(settleLayouts(replaceLayout(savedLayouts(), index, changed), index));
  };

  const movedPlaceholder = () => {
    const current = gesture();
    return current?.kind === "move" ? shownLayouts()[current.index] : undefined;
  };
  const gridColumnIndexes = () => Array.from({ length: columnCount() }, (_, column) => column);

  return (
    <div
      ref={grid}
      class="relative grid"
      style={{
        "grid-template-columns": `repeat(${columnCount()}, minmax(0, 1fr))`,
        "grid-auto-rows": `${GRID_ROW_HEIGHT_PX}px`,
        gap: `${GRID_GAP_PX}px`,
        "min-height": `${Math.max(0, rowCount() * ROW_STEP_PX - GRID_GAP_PX)}px`,
      }}
    >
      <Show when={gesture()}>
        <div
          aria-hidden="true"
          class="pointer-events-none absolute inset-0 grid"
          style={{
            "grid-template-columns": `repeat(${columnCount()}, minmax(0, 1fr))`,
            "column-gap": `${GRID_GAP_PX}px`,
          }}
        >
          <For each={gridColumnIndexes()} keyed={false}>
            {() => <div class="bg-hover" style={{ mask: GRID_CELL_MASK }} />}
          </For>
        </div>
      </Show>
      <Show when={movedPlaceholder()}>
        {(layout) => (
          <div
            class="relative rounded-lg border-2 border-dashed border-accent bg-hover"
            style={placeOnGrid(layout())}
          />
        )}
      </Show>
      <For each={props.widgets} keyed={false}>
        {(widget, index) => {
          const layout = () => shownLayouts()[index] ?? widget().layout;
          const isMoving = () => gestureKindByIndex[index] === "move";
          const isResizing = () => gestureKindByIndex[index] === "resize";
          const resizedLayout = () => (isResizing() ? gesture()?.layouts[index] : undefined);
          const followPointer = () => {
            const current = isMoving() ? gesture() : undefined;
            if (current?.kind !== "move") return undefined;
            const pointer = pointerInGrid();
            const x = pointer.x - current.grabOffset.x - layout().column * columnStepPx();
            const y = pointer.y - current.grabOffset.y - layout().row * ROW_STEP_PX;
            return `translate(${x}px, ${y}px)`;
          };
          return (
            <div
              class={[
                "group/widget @container/widget relative min-w-0",
                {
                  "[&_header]:cursor-grab [&_header]:touch-none": isArrangeable(),
                  "z-20 rounded-lg opacity-90 shadow-popup select-none [&_header]:cursor-grabbing":
                    isMoving(),
                  "z-20 rounded-lg ring-2 ring-accent select-none": isResizing(),
                },
              ]}
              style={{ ...placeOnGrid(layout()), transform: followPointer() }}
              onPointerDown={(e) => startMove(index, e)}
              onPointerMove={continueGesture}
              onPointerUp={endGesture}
              onPointerCancel={() => setGesture(undefined)}
            >
              <DashboardWidget
                widget={widget()}
                range={props.range}
                actions={
                  <DashboardPageWidgetActions
                    onEdit={() => props.onEdit(index)}
                    onDuplicate={() => props.onDuplicate(index)}
                    onRemove={() => props.onRemove(index)}
                  />
                }
              />
              <Show when={isArrangeable()}>
                <button
                  type="button"
                  aria-label={`Resize ${describeWidgetTitle(widget())}, ${widget().layout.width} of ${WIDE_GRID_COLUMNS} columns and ${widget().layout.height} rows: the arrow keys resize it, and with Shift move it`}
                  title="Drag to resize"
                  class="absolute right-0.5 bottom-0.5 z-10 flex size-5 cursor-se-resize touch-none items-center justify-center rounded text-muted opacity-0 transition-opacity group-hover/widget:opacity-100 pointer-coarse:opacity-100 hover:bg-hover hover:text-ink focus-visible:opacity-100"
                  onPointerDown={(e) => startResize(index, "corner", e)}
                  onKeyDown={(e) => changeLayoutWithKeys(index, e)}
                >
                  <ResizeIcon size={14} />
                </button>
                <div
                  aria-hidden="true"
                  class="absolute top-2 right-0 bottom-6 z-10 w-1.5 cursor-ew-resize touch-none rounded-full transition-colors hover:bg-accent/40"
                  onPointerDown={(e) => startResize(index, "right", e)}
                />
                <div
                  aria-hidden="true"
                  class="absolute right-6 bottom-0 left-2 z-10 h-1.5 cursor-ns-resize touch-none rounded-full transition-colors hover:bg-accent/40"
                  onPointerDown={(e) => startResize(index, "bottom", e)}
                />
              </Show>
              <Show when={resizedLayout()}>
                {(resized) => (
                  <div class="pointer-events-none absolute right-7 bottom-1 z-10 rounded bg-ink px-1.5 py-0.5 font-mono text-2xs text-surface">
                    {resized().width} × {resized().height}
                  </div>
                )}
              </Show>
            </div>
          );
        }}
      </For>
    </div>
  );
}

function toPointInWindow(e: PointerEvent): PointInWindow {
  return { clientX: e.clientX, clientY: e.clientY };
}

function replaceLayout(
  layouts: ReadonlyArray<WidgetLayout>,
  index: number,
  replacement: WidgetLayout,
): WidgetLayout[] {
  return layouts.map((layout, layoutIndex) => (layoutIndex === index ? replacement : layout));
}

function hasLayoutChanged(
  before: ReadonlyArray<WidgetLayout>,
  after: ReadonlyArray<WidgetLayout>,
): boolean {
  return JSON.stringify(before) !== JSON.stringify(after);
}

function placeOnGrid(layout: WidgetLayout): JSX.CSSProperties {
  return {
    "grid-column": `${layout.column + 1} / span ${layout.width}`,
    "grid-row": `${layout.row + 1} / span ${layout.height}`,
  };
}
