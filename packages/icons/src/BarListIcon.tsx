import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function BarListIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M2.8 3.6h10.4M2.8 8h7.4M2.8 12.4h4.4" {...LINE_STROKE} stroke-width="2.2" />
    </Icon>
  );
}
