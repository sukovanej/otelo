import { type Accessor, createMemo, createSignal } from "solid-js";

import { type ItemBox, moveValue, pickDropIndex, type ReorderAxis } from "./reorder";

// A press that moves less than this is a click, not a drag.
const DRAG_FROM_PX = 4;

const SETTLE_MS = 180;

export interface ReorderDrag<T extends string> {
  // The order the drop would leave, which the items show while the drag lasts.
  readonly shownOrder: Accessor<ReadonlyArray<T>>;
  readonly draggedValue: Accessor<T | undefined>;
  readonly ghost: Accessor<GhostPlacement<T> | undefined>;
  readonly start: (e: PointerEvent, container: HTMLElement) => void;
  readonly move: (e: PointerEvent) => void;
  readonly end: (e: PointerEvent) => boolean;
  readonly cancel: () => void;
}

export interface GhostPlacement<T extends string> {
  readonly value: T;
  readonly left: number;
  readonly top: number;
  readonly width: number;
}

interface ReorderDragOptions<T extends string> {
  readonly axis: ReorderAxis;
  readonly values: () => ReadonlyArray<T>;
  readonly onReorder: (values: T[]) => void;
}

interface DragState<T extends string> {
  readonly pointerId: number;
  readonly container: HTMLElement;
  readonly value: T;
  readonly startX: number;
  readonly startY: number;
  readonly grabOffsetX: number;
  readonly grabOffsetY: number;
  readonly width: number;
  readonly isMoving: boolean;
  readonly ghostLeft: number;
  readonly ghostTop: number;
  readonly order: ReadonlyArray<T>;
}

// The items of a container carry `data-reorder-value`, and a part of one that
// carries `data-reorder-ignore`, such as its remove button, starts no drag. The
// container is the offset parent of its items.
export function createReorderDrag<T extends string>(
  options: ReorderDragOptions<T>,
): ReorderDrag<T> {
  const [state, setState] = createSignal<DragState<T>>();
  const movingState = createMemo(() => {
    const current = state();
    return current?.isMoving ? current : undefined;
  });

  const start = (e: PointerEvent, container: HTMLElement) => {
    const pressed = e.target;
    if (
      e.button !== 0 ||
      !(pressed instanceof Element) ||
      pressed.closest("[data-reorder-ignore]")
    ) {
      return;
    }
    const item = pressed.closest<HTMLElement>("[data-reorder-value]");
    const value = options.values().find((known) => known === item?.dataset["reorderValue"]);
    if (!item || value === undefined || !container.contains(item)) return;
    e.preventDefault();
    container.setPointerCapture(e.pointerId);
    const itemBox = item.getBoundingClientRect();
    setState({
      pointerId: e.pointerId,
      container,
      value,
      startX: e.clientX,
      startY: e.clientY,
      grabOffsetX: e.clientX - itemBox.left,
      grabOffsetY: e.clientY - itemBox.top,
      width: itemBox.width,
      isMoving: false,
      ghostLeft: 0,
      ghostTop: 0,
      order: options.values(),
    });
  };
  const move = (e: PointerEvent) => {
    const current = state();
    if (current?.pointerId !== e.pointerId) return;
    const distance = Math.hypot(e.clientX - current.startX, e.clientY - current.startY);
    if (!current.isMoving && distance < DRAG_FROM_PX) return;
    const point = toPointInContainer(current.container, e);
    // The layout boxes, not the drawn ones, so items still sliding to their
    // place do not move the drop back and forth.
    const otherItems = listItems(current.container)
      .filter((item) => item.dataset["reorderValue"] !== current.value)
      .map(measureLayoutBox);
    const dropIndex = pickDropIndex(options.axis, otherItems, point.x, point.y);
    const order = moveValue(options.values(), current.value, dropIndex);
    const isReordered = order.some((value, index) => value !== current.order[index]);
    const boxesBefore = isReordered ? mapItemBoxes(current.container) : undefined;
    setState({
      ...current,
      isMoving: true,
      ghostLeft: point.x - current.grabOffsetX,
      ghostTop: point.y - current.grabOffsetY,
      order,
    });
    if (boxesBefore) settleItems(current.container, boxesBefore);
  };
  const end = (e: PointerEvent) => {
    const current = state();
    if (current?.pointerId !== e.pointerId) return false;
    setState(undefined);
    if (!current.isMoving) return false;
    const containerBox = current.container.getBoundingClientRect();
    const ghostBox = {
      left: containerBox.left + current.container.clientLeft + current.ghostLeft,
      top: containerBox.top + current.container.clientTop + current.ghostTop,
      right: 0,
      bottom: 0,
    };
    const values = options.values();
    if (current.order.some((value, index) => value !== values[index])) {
      options.onReorder([...current.order]);
    }
    settleItems(current.container, new Map([[current.value, ghostBox]]));
    return true;
  };

  return {
    shownOrder: () => state()?.order ?? options.values(),
    draggedValue: () => movingState()?.value,
    ghost: () => {
      const current = movingState();
      return (
        current && {
          value: current.value,
          left: current.ghostLeft,
          top: current.ghostTop,
          width: current.width,
        }
      );
    },
    start,
    move,
    end,
    cancel: () => setState(undefined),
  };
}

// Slides each item from where it was drawn to where the new order puts it, so
// the eye follows it. Solid draws the new order in a microtask, and this one
// runs after it.
export function settleItems(container: HTMLElement, boxesBefore: ReadonlyMap<string, ItemBox>) {
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  queueMicrotask(() => {
    for (const item of listItems(container)) {
      const before = boxesBefore.get(item.dataset["reorderValue"] ?? "");
      if (!before) continue;
      for (const animation of item.getAnimations()) animation.cancel();
      const after = item.getBoundingClientRect();
      const offsetX = before.left - after.left;
      const offsetY = before.top - after.top;
      if (offsetX === 0 && offsetY === 0) continue;
      item.animate(
        [{ transform: `translate(${offsetX}px, ${offsetY}px)` }, { transform: "none" }],
        { duration: SETTLE_MS, easing: "cubic-bezier(0.2, 0.8, 0.2, 1)" },
      );
    }
  });
}

// A focused element that the new order moves in the page loses the focus, so
// the moved item takes it back.
export function refocusItem(container: HTMLElement, value: string, selector: string) {
  setTimeout(() => {
    const item = listItems(container).find(
      (candidate) => candidate.dataset["reorderValue"] === value,
    );
    const target = item?.matches(selector) ? item : item?.querySelector<HTMLElement>(selector);
    target?.focus();
  }, 0);
}

export function mapItemBoxes(container: HTMLElement): Map<string, ItemBox> {
  return new Map(
    listItems(container).map((item) => [
      item.dataset["reorderValue"] ?? "",
      item.getBoundingClientRect(),
    ]),
  );
}

function toPointInContainer(container: HTMLElement, e: PointerEvent) {
  const box = container.getBoundingClientRect();
  return {
    x: e.clientX - box.left - container.clientLeft,
    y: e.clientY - box.top - container.clientTop,
  };
}

function measureLayoutBox(item: HTMLElement): ItemBox {
  return {
    left: item.offsetLeft,
    right: item.offsetLeft + item.offsetWidth,
    top: item.offsetTop,
    bottom: item.offsetTop + item.offsetHeight,
  };
}

function listItems(container: HTMLElement): HTMLElement[] {
  return [...container.querySelectorAll<HTMLElement>("[data-reorder-value]")];
}
