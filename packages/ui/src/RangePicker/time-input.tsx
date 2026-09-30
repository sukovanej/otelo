import { ClockIcon } from "@otelo/icons";

import { control, cx, plain, sizes } from "../classes";
import { CLOCK_PART_STARTS, findClockPartAtCaret, parseClock, stepClock } from "../date";

const CLOCK_PART_LENGTH = 2;

interface TimeInputProps {
  readonly value: string;
  readonly onChange: (clock: string) => void;
  readonly label: string;
}

export default function TimeInput(props: TimeInputProps) {
  let input!: HTMLInputElement;

  const takeClock = (clock: string) => {
    input.value = clock;
    if (clock !== props.value) props.onChange(clock);
  };
  const readTypedClock = () => parseClock(input.value) ?? props.value;
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter") {
      // Not the form around the field: Enter takes the time.
      e.preventDefault();
      takeClock(readTypedClock());
    } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      e.preventDefault();
      const part = findClockPartAtCaret(input.selectionStart ?? 0);
      takeClock(stepClock(readTypedClock(), part, e.key === "ArrowUp" ? 1 : -1));
      const partStart = CLOCK_PART_STARTS[part];
      input.setSelectionRange(partStart, partStart + CLOCK_PART_LENGTH);
    }
  };

  return (
    <div
      class={cx(
        control,
        plain,
        sizes.md,
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
        onChange={() => takeClock(readTypedClock())}
        onKeyDown={onKeyDown}
      />
    </div>
  );
}
