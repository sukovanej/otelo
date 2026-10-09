import { createMemo, createSignal, flush, For, Show } from "solid-js";

import { CheckIcon, ChevronIcon, MinusIcon, PlusIcon } from "@otelo/icons";

import Button from "../Button";
import { cx, option, plain, popover, type Size, sizes } from "../classes";
import { moveListIndex, toListStep } from "../keys";
import { createPopover } from "../popover";
import {
  findPreset,
  formatHumanDuration,
  formatRangeLabel,
  moveRangeToNow,
  type Range,
  type RangeLabelPart,
  RANGE_PRESETS,
  resizeRange,
  resolveRange,
  shiftRange,
  splitRangeLabel,
  UNTIL_NOW,
} from "../range";
import DateTimeInput from "./date-time-input";

const FALLBACK_RANGE_MS = 3_600_000;

const SEGMENT_CLASSES = `${plain} relative -ml-px cursor-pointer border first:ml-0 first:rounded-l-md last:rounded-r-md enabled:hover:bg-hover focus-visible:z-10 disabled:cursor-default disabled:text-muted/50`;

const SHIFT_BUTTON_CLASSES: Record<Size, string> = {
  sm: "h-6 w-6",
  md: "h-8 w-7",
  lg: "h-10.5 w-9",
};

interface RangePickerProps {
  readonly since: string;
  readonly until: string;
  readonly onChange: (since: string, until: string) => void;
  readonly size?: Size | undefined;
}

export default function RangePicker(props: RangePickerProps) {
  let trigger!: HTMLButtonElement;
  let presetList!: HTMLUListElement;
  const pickerPopover = createPopover({ returnFocusTo: () => trigger });
  const [from, setFrom] = createSignal(new Date());
  const [to, setTo] = createSignal(new Date());

  const range = (): Range => ({ since: props.since, until: props.until });
  const endsNow = () => props.until === UNTIL_NOW;
  const pickedPreset = createMemo(() => findPreset(range()));
  const rangeLengthMs = () => {
    const resolved = resolveRange(range(), Date.now());
    return resolved && resolved.endMs - resolved.startMs;
  };
  const shiftLabelSuffix = () => {
    const lengthMs = rangeLengthMs();
    return lengthMs === undefined ? "" : ` by ${formatHumanDuration(lengthMs)}`;
  };
  const stepButtonClasses = () =>
    cx(
      SEGMENT_CLASSES,
      SHIFT_BUTTON_CLASSES[props.size ?? "md"],
      "flex items-center justify-center",
    );
  const describeResize = (step: 1 | -1) => {
    const direction = step === 1 ? "Longer" : "Shorter";
    const resized = resizeRange(range(), step, Date.now());
    return resized ? `${direction}: ${formatRangeLabel(resized, Date.now())}` : direction;
  };
  const changeRange = (next: Range | undefined) => {
    if (next) props.onChange(next.since, next.until);
  };

  const openPicker = () => {
    const nowMs = Date.now();
    const resolved = resolveRange(range(), nowMs) ?? {
      startMs: nowMs - FALLBACK_RANGE_MS,
      endMs: nowMs,
    };
    // The fields hold whole seconds.
    setFrom(new Date(Math.floor(resolved.startMs / 1000) * 1000));
    setTo(new Date(Math.floor(resolved.endMs / 1000) * 1000));
    pickerPopover.show();
    flush();
    (
      presetList.querySelector<HTMLElement>("[aria-current]") ?? presetList.querySelector("button")
    )?.focus();
  };
  const closePicker = () => {
    pickerPopover.hide();
    trigger.focus();
  };
  const onPresetKeyDown = (e: KeyboardEvent) => {
    const step = toListStep(e);
    if (step === 0) return;
    e.preventDefault();
    const buttons = [...presetList.querySelectorAll("button")];
    const focusedIndex = buttons.findIndex((button) => button === document.activeElement);
    buttons[moveListIndex(focusedIndex, step, buttons.length)]?.focus();
  };

  return (
    <div ref={pickerPopover.rootRef} class="relative flex items-center gap-1.5">
      <div class="flex">
        <button
          type="button"
          class={stepButtonClasses()}
          aria-label={`Earlier${shiftLabelSuffix()}`}
          title={`Earlier${shiftLabelSuffix()}`}
          disabled={rangeLengthMs() === undefined}
          onClick={() => changeRange(shiftRange(range(), -1, Date.now()))}
        >
          <ChevronIcon direction="left" size={13} />
        </button>
        <button
          type="button"
          class={stepButtonClasses()}
          aria-label={describeResize(-1)}
          title={describeResize(-1)}
          disabled={resizeRange(range(), -1, Date.now()) === undefined}
          onClick={() => changeRange(resizeRange(range(), -1, Date.now()))}
        >
          <MinusIcon size={13} />
        </button>
        <button
          ref={trigger}
          type="button"
          class={cx(SEGMENT_CLASSES, sizes[props.size ?? "md"], "group flex items-center gap-2")}
          aria-label={`Range: ${formatRangeLabel(range(), Date.now())}`}
          aria-haspopup="dialog"
          aria-expanded={pickerPopover.open() ? "true" : "false"}
          onClick={() => (pickerPopover.open() ? pickerPopover.hide() : openPicker())}
        >
          <span class="flex gap-1.5 whitespace-nowrap">
            <For
              each={keyLabelParts(splitRangeLabel(range(), Date.now()))}
              keyed={(part) => part.key}
            >
              {(part) => (
                <span class={part().dim ? "text-muted" : "font-medium tabular-nums"}>
                  {part().text}
                </span>
              )}
            </For>
          </span>
          <ChevronIcon size={13} class="text-muted group-aria-expanded:rotate-180" />
        </button>
        <button
          type="button"
          class={stepButtonClasses()}
          aria-label={describeResize(1)}
          title={describeResize(1)}
          disabled={resizeRange(range(), 1, Date.now()) === undefined}
          onClick={() => changeRange(resizeRange(range(), 1, Date.now()))}
        >
          <PlusIcon size={13} />
        </button>
        <button
          type="button"
          class={stepButtonClasses()}
          aria-label={`Later${shiftLabelSuffix()}`}
          title={endsNow() ? "The range ends now" : `Later${shiftLabelSuffix()}`}
          disabled={endsNow() || rangeLengthMs() === undefined}
          onClick={() => changeRange(shiftRange(range(), 1, Date.now()))}
        >
          <ChevronIcon direction="right" size={13} />
        </button>
      </div>
      <Button
        size={props.size}
        title={endsNow() ? "The range ends now" : "Move the range to end now"}
        disabled={endsNow() || rangeLengthMs() === undefined}
        onClick={() => changeRange(moveRangeToNow(range(), Date.now()))}
      >
        Now
      </Button>
      <Show when={pickerPopover.open()}>
        <div
          ref={pickerPopover.panelRef}
          class={cx(popover, "flex", pickerPopover.alignsRight() ? "right-0" : "left-0")}
          role="dialog"
          aria-label="Range"
        >
          <ul
            ref={presetList}
            class="m-0 list-none border-r border-line p-1"
            aria-label="Ranges that end now"
            onKeyDown={onPresetKeyDown}
          >
            <For each={RANGE_PRESETS}>
              {(preset) => (
                <li>
                  <button
                    type="button"
                    class={cx(
                      option,
                      "w-full items-center gap-2 pr-4 whitespace-nowrap hover:bg-active",
                    )}
                    aria-current={pickedPreset() === preset ? "true" : undefined}
                    onClick={() => {
                      closePicker();
                      props.onChange(preset.since, UNTIL_NOW);
                    }}
                  >
                    <CheckIcon
                      size={13}
                      class={cx("text-accent", pickedPreset() !== preset && "invisible")}
                    />
                    {preset.label}
                  </button>
                </li>
              )}
            </For>
          </ul>
          <div class="grid grid-cols-[auto_auto] items-center gap-x-3 gap-y-2 p-3">
            <span class="text-muted">From</span>
            <DateTimeInput label="From" value={from()} onChange={setFrom} />
            <span class="text-muted">To</span>
            <DateTimeInput label="To" value={to()} onChange={setTo} />
            <div class="col-span-2 flex items-center justify-end gap-3">
              <Show when={from() >= to()}>
                <span class="text-error" role="alert">
                  From has to be before To
                </span>
              </Show>
              <Button
                variant="primary"
                disabled={from() >= to()}
                onClick={() => {
                  closePicker();
                  props.onChange(from().toISOString(), to().toISOString());
                }}
              >
                Apply
              </Button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}

interface KeyedLabelPart extends RangeLabelPart {
  readonly key: string;
}

function keyLabelParts(parts: ReadonlyArray<RangeLabelPart>): KeyedLabelPart[] {
  return parts.map((part, index) => ({
    ...part,
    key: JSON.stringify([index, part.dim, part.text]),
  }));
}
