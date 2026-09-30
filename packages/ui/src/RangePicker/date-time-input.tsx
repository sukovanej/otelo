import { joinDateAndClock, toLocalClock, toLocalDate } from "../date";
import DateInput from "./date-input";
import TimeInput from "./time-input";

interface DateTimeInputProps {
  readonly value: Date;
  readonly onChange: (value: Date) => void;
  readonly label: string;
}

export default function DateTimeInput(props: DateTimeInputProps) {
  const date = () => toLocalDate(props.value);
  const clock = () => toLocalClock(props.value);
  return (
    <div class="flex gap-1.5" role="group" aria-label={props.label}>
      <DateInput
        label={`${props.label} date`}
        value={date()}
        onChange={(nextDate) => props.onChange(joinDateAndClock(nextDate, clock()))}
      />
      <TimeInput
        label={`${props.label} time`}
        value={clock()}
        onChange={(nextClock) => props.onChange(joinDateAndClock(date(), nextClock))}
      />
    </div>
  );
}
