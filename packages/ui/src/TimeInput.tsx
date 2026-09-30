import { ClockIcon } from "@otelo/icons";

import { control, cx, plain, type Size, sizes } from "./classes";
import { clockPart, parseClock, stepClock } from "./date";

/**
 * A time of day, as `14:03:07`, to type as that, `14:03`, `14`, or `1403`.
 * The up and down arrows step the hours, the minutes, or the seconds that
 * the caret is in. Enter or leaving the field takes what is typed, and text
 * that is no time goes back to the time.
 */
export default function TimeInput(props: {
  value: string;
  onChange: (clock: string) => void;
  /** The name of the time, for screen readers. */
  label: string;
  size?: Size | undefined;
}) {
  let input!: HTMLInputElement;

  const take = (clock: string) => {
    input.value = clock;
    if (clock !== props.value) props.onChange(clock);
  };
  const typed = () => parseClock(input.value) ?? props.value;
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter") {
      // Not the form around the field: Enter takes the time.
      e.preventDefault();
      take(typed());
    } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      e.preventDefault();
      const part = clockPart(input.selectionStart ?? 0);
      take(stepClock(typed(), part, e.key === "ArrowUp" ? 1 : -1));
      input.setSelectionRange(part * 3, part * 3 + 2);
    }
  };

  return (
    <div
      class={cx(
        control,
        plain,
        sizes[props.size ?? "md"],
        "flex items-center gap-1.5 focus-within:border-line-focus",
      )}
    >
      <ClockIcon size={14} class="text-muted" />
      <input
        ref={input}
        class="w-[8ch] bg-transparent font-mono focus:outline-none"
        aria-label={props.label}
        placeholder="hh:mm:ss"
        inputmode="numeric"
        autocomplete="off"
        value={props.value}
        onChange={() => take(typed())}
        onKeyDown={onKeyDown}
      />
    </div>
  );
}
