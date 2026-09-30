import { createSignal, For, Show } from "solid-js";

import { CheckIcon, ChevronIcon } from "@otelo/icons";

import Button from "./Button";
import {
  cx,
  option as optionClass,
  plain,
  popover as popoverClass,
  type Size,
  sizes,
} from "./classes";
import DateTimeInput from "./DateTimeInput";
import { listStep, move } from "./keys";
import { createPopover } from "./popover";
import {
  humanDuration,
  presetOf,
  PRESETS,
  type Range,
  rangeLabel,
  rangeParts,
  resolve,
  shift,
  toNow,
} from "./range";

/** The custom range starts as the last hour when the range does not read. */
const FALLBACK_MS = 3_600_000;

/** One of the buttons in a row that share their borders. */
const segment = `${plain} relative -ml-px cursor-pointer border first:ml-0 first:rounded-l-md last:rounded-r-md enabled:hover:bg-hover focus-visible:z-10 disabled:cursor-default disabled:text-muted/50`;

/** The height and the width of a button that shifts the range. */
const arrows = {
  sm: "h-6 w-6",
  md: "h-8 w-7",
  lg: "h-10.5 w-9",
} as const;

/**
 * The range of a query: `since` and `until`, each a duration before now such
 * as `2h` or an RFC 3339 timestamp, and an empty `until` for now. The button
 * names the range and opens the ranges that end now and a custom range from
 * one local time to another. The arrows beside it shift the range by its own
 * length, and Now moves it to end now.
 */
export default function RangePicker(props: {
  since: string;
  until: string;
  onChange: (since: string, until: string) => void;
  size?: Size | undefined;
}) {
  let trigger!: HTMLButtonElement;
  let presets!: HTMLUListElement;
  const popover = createPopover({ focus: () => trigger });
  const [from, setFrom] = createSignal(new Date());
  const [to, setTo] = createSignal(new Date());

  const range = (): Range => ({ since: props.since, until: props.until });
  const endsNow = () => props.until === "";
  const length = () => {
    const span = resolve(range(), Date.now());
    return span && span.end - span.start;
  };
  const by = () => {
    const ms = length();
    return ms === undefined ? "" : ` by ${humanDuration(ms)}`;
  };
  const change = (next: Range | undefined) => {
    if (next) props.onChange(next.since, next.until);
  };

  const show = () => {
    const now = Date.now();
    const span = resolve(range(), now) ?? { start: now - FALLBACK_MS, end: now };
    // The fields hold whole seconds.
    setFrom(new Date(Math.floor(span.start / 1000) * 1000));
    setTo(new Date(Math.floor(span.end / 1000) * 1000));
    popover.show();
    (
      presets.querySelector<HTMLElement>("[aria-current]") ?? presets.querySelector("button")
    )?.focus();
  };
  const close = () => {
    popover.hide();
    trigger.focus();
  };
  const onPresetKey = (e: KeyboardEvent) => {
    const step = listStep(e);
    if (step === 0) return;
    e.preventDefault();
    const buttons = [...presets.querySelectorAll("button")];
    const focused = buttons.findIndex((button) => button === document.activeElement);
    buttons[move(focused, step, buttons.length)]?.focus();
  };

  return (
    <div ref={popover.root} class="relative flex items-center gap-1.5">
      <div class="flex">
        <button
          type="button"
          class={cx(segment, arrows[props.size ?? "md"], "flex items-center justify-center")}
          aria-label={`Earlier${by()}`}
          title={`Earlier${by()}`}
          disabled={length() === undefined}
          onClick={() => change(shift(range(), -1, Date.now()))}
        >
          <ChevronIcon direction="left" size={13} />
        </button>
        <button
          ref={trigger}
          type="button"
          class={cx(segment, sizes[props.size ?? "md"], "group flex items-center gap-2")}
          aria-label={`Range: ${rangeLabel(range(), Date.now())}`}
          aria-haspopup="dialog"
          aria-expanded={popover.open()}
          onClick={() => (popover.open() ? popover.hide() : show())}
        >
          <span class="flex gap-1.5 whitespace-nowrap">
            <For each={rangeParts(range(), Date.now())}>
              {(part) => (
                <span class={part.dim ? "text-muted" : "font-medium tabular-nums"}>
                  {part.text}
                </span>
              )}
            </For>
          </span>
          <ChevronIcon size={13} class="text-muted group-aria-expanded:rotate-180" />
        </button>
        <button
          type="button"
          class={cx(segment, arrows[props.size ?? "md"], "flex items-center justify-center")}
          aria-label={`Later${by()}`}
          title={endsNow() ? "The range ends now" : `Later${by()}`}
          disabled={endsNow() || length() === undefined}
          onClick={() => change(shift(range(), 1, Date.now()))}
        >
          <ChevronIcon direction="right" size={13} />
        </button>
      </div>
      <Button
        size={props.size}
        title={endsNow() ? "The range ends now" : "Move the range to end now"}
        disabled={endsNow() || length() === undefined}
        onClick={() => change(toNow(range(), Date.now()))}
      >
        Now
      </Button>
      <Show when={popover.open()}>
        <div
          ref={popover.panel}
          class={cx(popoverClass, "flex", popover.atEnd() ? "right-0" : "left-0")}
          role="dialog"
          aria-label="Range"
        >
          <ul
            ref={presets}
            class="m-0 list-none border-r border-line p-1"
            aria-label="Ranges that end now"
            onKeyDown={onPresetKey}
          >
            <For each={PRESETS}>
              {(preset) => (
                <li>
                  <button
                    type="button"
                    class={cx(
                      optionClass,
                      "w-full items-center gap-2 pr-4 whitespace-nowrap hover:bg-active",
                    )}
                    aria-current={presetOf(range()) === preset ? "true" : undefined}
                    onClick={() => {
                      close();
                      props.onChange(preset.since, "");
                    }}
                  >
                    <CheckIcon
                      size={13}
                      class={cx("text-accent", presetOf(range()) !== preset && "invisible")}
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
                  close();
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
