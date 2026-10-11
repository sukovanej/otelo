import { formatMillisOfSecond, formatTimeToSecond, parseTime } from "./time";

interface RowTimeProps {
  readonly at: string;
}

export default function RowTime(props: RowTimeProps) {
  const date = () => parseTime(props.at);
  return (
    <time class="whitespace-nowrap" datetime={props.at}>
      {formatTimeToSecond(date())}
      <span class="max-md:hidden">.{formatMillisOfSecond(date())}</span>
    </time>
  );
}
