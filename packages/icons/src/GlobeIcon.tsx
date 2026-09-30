import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function GlobeIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g {...LINE_STROKE} stroke-width="1.2">
        <circle cx="8" cy="8" r="6.2" />
        <ellipse cx="8" cy="8" rx="2.6" ry="6.2" />
        <path d="M1.8 8h12.4M2.9 4.9h10.2M2.9 11.1h10.2" />
      </g>
    </Icon>
  );
}
