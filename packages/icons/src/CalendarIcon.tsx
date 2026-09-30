import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function CalendarIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g {...LINE_STROKE} stroke-width="1.2">
        <rect x="2.2" y="3.2" width="11.6" height="10.6" rx="1.8" />
        <path d="M2.2 6.6h11.6M5.4 1.8v2.6M10.6 1.8v2.6" />
      </g>
    </Icon>
  );
}
