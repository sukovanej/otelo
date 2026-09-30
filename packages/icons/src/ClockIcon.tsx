import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function ClockIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g {...LINE_STROKE} stroke-width="1.2">
        <circle cx="8" cy="8" r="6.2" />
        <path d="M8 4.6V8l2.3 1.5" />
      </g>
    </Icon>
  );
}
