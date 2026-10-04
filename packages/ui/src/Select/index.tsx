import {
  createMemo,
  createProjection,
  createSignal,
  createUniqueId,
  flush,
  For,
  Show,
} from "solid-js";

import { ChevronIcon, CloseIcon } from "@otelo/icons";

import { control, cx, plain, popover, type Size, sizes } from "../classes";
import { moveListIndex, toListStep } from "../keys";
import { createPopover } from "../popover";
import { moveValue } from "./reorder";
import { createReorderDrag, mapItemBoxes, refocusItem, settleItems } from "./reorder-drag";
import SelectChosenList from "./select-chosen-list";
import SelectListOption from "./select-list-option";
import { groupSelectOptions, type SelectOption } from "./select-options";
import SelectTag from "./select-tag";

export type { SelectOption } from "./select-options";

const SEARCH_FROM_OPTION_COUNT = 9;

const FIELD_SIZES: Record<Size, string> = {
  sm: "min-h-6 px-1.5 py-px text-2xs",
  md: "min-h-8 px-2.5 py-[3px]",
  lg: "min-h-10.5 px-3.5 py-1.5",
};

type SelectProps<T extends string> = SingleSelectProps<T> | MultipleSelectProps<T>;

interface SelectBaseProps<T extends string> {
  readonly label: string;
  readonly options: ReadonlyArray<SelectOption<T>>;
  readonly placeholder?: string;
  // Turns what the search holds into a value the options lack; without it only
  // the options can be picked.
  readonly typedValue?: (text: string) => T | undefined;
  readonly stretch?: boolean;
  readonly size?: Size | undefined;
}

interface SingleSelectProps<T extends string> extends SelectBaseProps<T> {
  readonly selection: "single";
  readonly value: T;
  readonly onChange: (value: T) => void;
}

interface MultipleSelectProps<T extends string> extends SelectBaseProps<T> {
  readonly selection: "multiple";
  readonly values: ReadonlyArray<T>;
  readonly onChange: (values: T[]) => void;
}

// A select with few options is a button. One that a search helps, and every
// multiple one, is a field the search is typed into, with the list below it.
export default function Select<T extends string>(props: SelectProps<T>) {
  let trigger!: HTMLElement;
  let optionList!: HTMLDivElement;
  const listPopover = createPopover({ returnFocusTo: () => trigger });
  const [search, setSearch] = createSignal("");
  let field: HTMLDivElement | undefined;
  let hasDraggedTag = false;
  const chosenValues = (): ReadonlyArray<T> =>
    props.selection === "single" ? [props.value] : props.values;
  const listboxId = createUniqueId();
  const chosenByValue = createProjection<Partial<Record<string, true>>>(
    () => Object.fromEntries(chosenValues().map((value) => [value, true])),
    {},
  );
  const isField = () =>
    props.selection === "multiple" ||
    props.typedValue !== undefined ||
    props.options.length >= SEARCH_FROM_OPTION_COUNT;
  const listedOptions = createMemo(() =>
    props.selection === "multiple"
      ? props.options.filter((option) => !props.values.includes(option.value))
      : props.options,
  );
  const groups = createMemo(() => groupSelectOptions(listedOptions(), search()));
  const typedOption = createMemo((): SelectOption<T> | undefined => {
    const text = search().trim();
    const value = text === "" ? undefined : props.typedValue?.(text);
    if (
      value === undefined ||
      chosenValues().includes(value) ||
      props.options.some((known) => known.value === value)
    ) {
      return undefined;
    }
    return { value, label: `Use ${text}` };
  });
  const findOptionLabel = (value: T) =>
    props.options.find((known) => known.value === value)?.label ?? value;
  const chosenText = () => {
    const values = chosenValues().filter((value) => value !== "");
    return values.length === 0
      ? undefined
      : values.map((value) => findOptionLabel(value)).join(", ");
  };
  const reorderValues = (values: T[]) => {
    if (props.selection === "multiple") props.onChange(values);
  };
  const tagDrag = createReorderDrag({
    axis: "inline",
    values: () => (props.selection === "multiple" ? props.values : []),
    onReorder: reorderValues,
  });
  const chosenTags = createMemo(() =>
    props.selection === "multiple"
      ? tagDrag.shownOrder().map((value) => ({
          value,
          label: findOptionLabel(value),
        }))
      : [],
  );
  // A single field shows the chosen label until the list opens for a search.
  const fieldText = () =>
    props.selection === "single" && !listPopover.open() ? (chosenText() ?? "") : search();
  const fieldPlaceholder = () => {
    if (props.selection === "multiple") {
      return chosenTags().length === 0 ? (props.placeholder ?? "") : "";
    }
    return listPopover.open()
      ? (chosenText() ?? props.placeholder ?? "")
      : (props.placeholder ?? "");
  };
  const listOptionElements = () => [...optionList.querySelectorAll<HTMLElement>("[role=option]")];

  const openList = () => {
    setSearch("");
    listPopover.show();
    if (isField()) return;
    flush();
    const elements = listOptionElements();
    const chosenIndex = elements.findIndex((element) => element.ariaSelected === "true");
    elements[Math.max(0, chosenIndex)]?.focus();
  };
  const pickValue = (value: T) => {
    if (props.selection === "single") {
      listPopover.hide();
      trigger.focus();
      if (value !== props.value) props.onChange(value);
      return;
    }
    // The search clears before the parent hears of the pick, so a fetch the
    // pick starts does not hold the clearing back past what is typed next.
    setSearch("");
    flush();
    trigger.focus();
    props.onChange(
      props.values.includes(value)
        ? props.values.filter((chosen) => chosen !== value)
        : [...props.values, value],
    );
  };
  const removeValue = (value: T) => {
    if (props.selection !== "multiple") return;
    props.onChange(props.values.filter((chosen) => chosen !== value));
  };
  const moveTag = (value: T, step: -1 | 1) => {
    if (props.selection !== "multiple" || !field) return;
    const boxesBefore = mapItemBoxes(field);
    props.onChange(moveValue(props.values, value, props.values.indexOf(value) + step));
    settleItems(field, boxesBefore);
    refocusItem(field, value, "[data-reorder-value]");
  };
  const onButtonKeyDown = (e: KeyboardEvent) => {
    if (toListStep(e) === 0 || listPopover.open()) return;
    e.preventDefault();
    openList();
  };
  const onFieldInput = (e: InputEvent & { currentTarget: HTMLInputElement }) => {
    // Typing into a closed single field starts a search of what is typed, not
    // of the chosen label it shows.
    if (props.selection === "single" && !listPopover.open()) {
      e.currentTarget.value = e.inputType.startsWith("insert") ? (e.data ?? "") : "";
    }
    setSearch(e.currentTarget.value);
    listPopover.show();
  };
  const onFieldKeyDown = (e: KeyboardEvent) => {
    const lastValue = chosenTags().at(-1)?.value;
    if (e.key === "Enter") {
      e.preventDefault();
      const firstValue = groups()[0]?.options[0]?.value ?? typedOption()?.value;
      if (search().trim() !== "" && firstValue !== undefined) pickValue(firstValue);
      else if (props.selection === "single") listPopover.hide();
    } else if (toListStep(e) === 1) {
      e.preventDefault();
      if (!listPopover.open()) setSearch("");
      listPopover.show();
      flush();
      listOptionElements()[0]?.focus();
    } else if (e.key === "Backspace" && search() === "" && lastValue !== undefined) {
      e.preventDefault();
      removeValue(lastValue);
    }
  };
  const onListKeyDown = (e: KeyboardEvent) => {
    const elements = listOptionElements();
    const focusedIndex = elements.findIndex((element) => element === document.activeElement);
    const step = toListStep(e);
    if (step === -1 && focusedIndex === 0 && isField()) {
      e.preventDefault();
      trigger.focus();
    } else if (step !== 0) {
      e.preventDefault();
      elements[moveListIndex(focusedIndex, step, elements.length)]?.focus();
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      elements[e.key === "Home" ? 0 : elements.length - 1]?.focus();
    } else if ((e.key === "Enter" || e.key === " ") && e.target instanceof HTMLElement) {
      const value = e.target.dataset["value"];
      const picked = [...(typedOption() ? [typedOption()] : []), ...props.options].find(
        (candidate) => candidate?.value === value,
      );
      if (!picked) return;
      e.preventDefault();
      pickValue(picked.value);
    }
  };

  return (
    <div ref={listPopover.rootRef} class={["relative", { "w-full": props.stretch }]}>
      <Show
        when={isField()}
        fallback={
          <button
            ref={(button) => {
              trigger = button;
            }}
            type="button"
            class={cx(
              control,
              plain,
              sizes[props.size ?? "md"],
              "group flex cursor-pointer items-center gap-2 hover:bg-hover",
              props.stretch ? "w-full" : "max-w-96",
            )}
            aria-haspopup="listbox"
            aria-controls={listboxId}
            aria-expanded={listPopover.open() ? "true" : "false"}
            onClick={() => (listPopover.open() ? listPopover.hide() : openList())}
            onKeyDown={onButtonKeyDown}
          >
            <span class="shrink-0 text-muted">{props.label}</span>
            <span
              class={[
                "min-w-0 flex-1 truncate text-left",
                { "text-muted": chosenText() === undefined },
              ]}
            >
              {chosenText() ?? props.placeholder ?? ""}
            </span>
            <ChevronIcon size={13} class="shrink-0 text-muted group-aria-expanded:rotate-180" />
          </button>
        }
      >
        <div
          class={cx(
            control,
            plain,
            FIELD_SIZES[props.size ?? "md"],
            "group relative flex cursor-text flex-wrap items-center gap-1 focus-within:border-line-focus",
            props.stretch ? "w-full" : "max-w-96",
          )}
          ref={(element) => {
            field = element;
          }}
          aria-expanded={listPopover.open() ? "true" : "false"}
          onClick={() => {
            if (hasDraggedTag) {
              hasDraggedTag = false;
              return;
            }
            trigger.focus();
            if (!listPopover.open()) openList();
          }}
          onPointerDown={(e) => {
            if (field) tagDrag.start(e, field);
          }}
          onPointerMove={(e) => tagDrag.move(e)}
          onPointerUp={(e) => {
            hasDraggedTag = tagDrag.end(e);
          }}
          onPointerCancel={() => tagDrag.cancel()}
        >
          <span class="mr-1 shrink-0 text-muted">{props.label}</span>
          <For each={chosenTags()} keyed={(tag) => tag.value}>
            {(tag) => (
              <SelectTag
                label={tag().label}
                value={tag().value}
                position={chosenTags().findIndex((chosen) => chosen.value === tag().value) + 1}
                count={chosenTags().length}
                dragged={tagDrag.draggedValue() === tag().value}
                onMove={(step) => moveTag(tag().value, step)}
                onRemove={() => removeValue(tag().value)}
              />
            )}
          </For>
          <input
            ref={(input) => {
              trigger = input;
            }}
            role="combobox"
            class="h-5.5 w-16 min-w-0 flex-1 truncate bg-transparent placeholder:text-muted focus:outline-none"
            aria-label={props.label}
            aria-expanded={listPopover.open() ? "true" : "false"}
            aria-controls={listboxId}
            aria-autocomplete="list"
            placeholder={fieldPlaceholder()}
            value={fieldText()}
            onInput={onFieldInput}
            onKeyDown={onFieldKeyDown}
          />
          <Show when={props.selection === "single"}>
            <ChevronIcon size={13} class="shrink-0 text-muted group-aria-expanded:rotate-180" />
          </Show>
          <Show when={tagDrag.ghost()}>
            {(ghost) => (
              <span
                class="pointer-events-none absolute z-10 flex h-5.5 items-center gap-0.5 rounded bg-active pr-0.5 pl-1.5 shadow-popup ring-1 ring-line"
                style={{ left: `${ghost().left}px`, top: `${ghost().top}px` }}
              >
                <span class="-translate-y-px">{findOptionLabel(ghost().value)}</span>
                <span class="flex size-4 translate-y-px items-center justify-center text-muted">
                  <CloseIcon size={10} />
                </span>
              </span>
            )}
          </Show>
        </div>
      </Show>
      <Show when={listPopover.open()}>
        <div
          ref={listPopover.panelRef}
          class={cx(
            popover,
            "flex max-h-80 min-w-full flex-col overflow-hidden",
            listPopover.alignsRight() ? "right-0" : "left-0",
          )}
        >
          <Show when={chosenTags().length > 1 && search() === ""}>
            <SelectChosenList
              label={props.label}
              items={chosenTags()}
              onReorder={reorderValues}
              onRemove={removeValue}
            />
          </Show>
          <div
            ref={optionList}
            class="min-h-0 overflow-y-auto p-1"
            id={listboxId}
            role="listbox"
            aria-label={props.label}
            aria-multiselectable={props.selection === "multiple" ? "true" : undefined}
            onKeyDown={onListKeyDown}
          >
            <For
              each={groups()}
              keyed={false}
              fallback={
                <Show when={!typedOption()}>
                  <div class="px-2 py-1 text-muted">No match</div>
                </Show>
              }
            >
              {(group) => (
                <div role="group" aria-label={group().title || undefined}>
                  <Show when={group().title}>
                    <div class="px-2 pt-1.5 pb-0.5 text-2xs font-semibold tracking-[0.04em] text-muted uppercase">
                      {group().title}
                    </div>
                  </Show>
                  <For each={group().options} keyed={false}>
                    {(groupOption) => (
                      <SelectListOption
                        option={groupOption()}
                        chosen={chosenByValue[groupOption().value] === true}
                        onPick={() => pickValue(groupOption().value)}
                      />
                    )}
                  </For>
                </div>
              )}
            </For>
            <Show when={typedOption()}>
              {(typed) => (
                <SelectListOption
                  option={typed()}
                  chosen={false}
                  onPick={() => pickValue(typed().value)}
                />
              )}
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
}
