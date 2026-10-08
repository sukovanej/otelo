import { For, Show } from "solid-js";

import { CloseIcon, GripIcon } from "@otelo/icons";

import { moveValue } from "./reorder";
import { createReorderDrag, mapItemBoxes, refocusItem, settleItems } from "./reorder-drag";

interface ChosenItem<T extends string> {
  readonly value: T;
  readonly label: string;
}

interface SelectChosenListProps<T extends string> {
  readonly label: string;
  readonly items: ReadonlyArray<ChosenItem<T>>;
  readonly onReorder: (values: T[]) => void;
  readonly onRemove: (value: T) => void;
}

// The chosen values as rows to drag into order, for a select whose order means
// something, such as the names a query groups by.
export default function SelectChosenList<T extends string>(props: SelectChosenListProps<T>) {
  let list!: HTMLDivElement;
  const values = () => props.items.map((item) => item.value);
  const labelOf = (value: T) => props.items.find((item) => item.value === value)?.label ?? value;
  const shownItems = () => drag.shownOrder().map((value) => ({ value, label: labelOf(value) }));
  const drag = createReorderDrag({
    axis: "block",
    values,
    onReorder: (reordered) => props.onReorder(reordered),
  });
  const moveWithKeys = (value: T, e: KeyboardEvent) => {
    const step = e.key === "ArrowUp" ? -1 : e.key === "ArrowDown" ? 1 : 0;
    if (step === 0) return;
    e.preventDefault();
    e.stopPropagation();
    const index = values().indexOf(value) + step;
    if (index < 0 || index >= values().length) return;
    const boxesBefore = mapItemBoxes(list);
    props.onReorder(moveValue(values(), value, index));
    settleItems(list, boxesBefore);
    refocusItem(list, value, "button");
  };

  return (
    <div class="border-b border-line p-1">
      <div class="px-2 pt-1.5 pb-0.5 text-2xs font-semibold tracking-[0.04em] text-muted uppercase">
        {props.label}
      </div>
      <div
        ref={list}
        role="list"
        aria-label={props.label}
        class="relative"
        onPointerDown={(e) => drag.start(e, list)}
        onPointerMove={(e) => drag.move(e)}
        onPointerUp={(e) => drag.end(e)}
        onPointerCancel={() => drag.cancel()}
      >
        <For each={shownItems()} keyed={(item) => item.value}>
          {(item) => {
            const position = () => drag.shownOrder().indexOf(item().value) + 1;
            return (
              <div
                role="listitem"
                data-reorder-value={item().value}
                class={[
                  "flex h-7 cursor-grab touch-none items-center gap-1.5 rounded-[5px] pr-1 select-none",
                  drag.draggedValue() === item().value
                    ? "border border-dashed border-accent bg-accent/5 *:invisible"
                    : "hover:bg-hover",
                ]}
              >
                <button
                  type="button"
                  aria-label={`Move ${item().label}, ${position()} of ${props.items.length}: the up and down keys move it`}
                  class="flex h-6 w-5 shrink-0 cursor-grab items-center justify-center rounded-sm text-muted outline-none hover:text-ink focus-visible:ring-2 focus-visible:ring-line-focus"
                  onKeyDown={(e) => moveWithKeys(item().value, e)}
                >
                  <GripIcon size={14} />
                </button>
                <span class="min-w-0 flex-1 truncate pl-1">{item().label}</span>
                <button
                  type="button"
                  data-reorder-ignore
                  aria-label={`Remove ${item().label}`}
                  class="flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-sm text-muted hover:bg-active hover:text-ink"
                  onClick={() => props.onRemove(item().value)}
                >
                  <CloseIcon size={10} />
                </button>
              </div>
            );
          }}
        </For>
        <Show when={drag.ghost()}>
          {(ghost) => (
            <div
              class="pointer-events-none absolute z-10 flex h-7 items-center gap-1.5 rounded-[5px] border border-line bg-surface pr-1 shadow-popup"
              style={{
                left: `${drag.ghostPoint().left}px`,
                top: `${drag.ghostPoint().top}px`,
                width: `${ghost().width}px`,
              }}
            >
              <span class="flex w-5 shrink-0 justify-center text-muted">
                <GripIcon size={14} />
              </span>
              <span class="min-w-0 flex-1 truncate pl-1">{labelOf(ghost().value)}</span>
            </div>
          )}
        </Show>
      </div>
    </div>
  );
}
