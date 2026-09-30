import type { Size } from "./classes";
import { at, clockOf, dateOf } from "./date";
import DateInput from "./DateInput";
import TimeInput from "./TimeInput";

/** A local date and time to the second: a `DateInput` and a `TimeInput`. */
export default function DateTimeInput(props: {
  value: Date;
  onChange: (value: Date) => void;
  /** The name of the time, such as `From`, for screen readers. */
  label: string;
  size?: Size | undefined;
}) {
  const date = () => dateOf(props.value);
  const clock = () => clockOf(props.value);
  return (
    <div class="flex gap-1.5" role="group" aria-label={props.label}>
      <DateInput
        label={`${props.label} date`}
        size={props.size}
        value={date()}
        onChange={(value) => props.onChange(at(value, clock()))}
      />
      <TimeInput
        label={`${props.label} time`}
        size={props.size}
        value={clock()}
        onChange={(value) => props.onChange(at(date(), value))}
      />
    </div>
  );
}
