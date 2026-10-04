import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function DashboardsIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g {...LINE_STROKE} stroke-width="1.2">
        <rect x="2.2" y="2.2" width="5" height="6.4" rx="1" />
        <rect x="8.8" y="2.2" width="5" height="3.6" rx="1" />
        <rect x="2.2" y="10.2" width="5" height="3.6" rx="1" />
        <rect x="8.8" y="7.4" width="5" height="6.4" rx="1" />
      </g>
    </Icon>
  );
}
