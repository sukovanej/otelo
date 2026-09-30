import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function ServicesIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g {...LINE_STROKE} stroke-width="1.2">
        <path d="M8 1.7 13.5 4.8v6.4L8 14.3 2.5 11.2V4.8Z" />
        <path d="M2.7 4.9 8 8l5.3-3.1M8 8v6.1" />
      </g>
    </Icon>
  );
}
