import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function LineChartIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M2 13.6h12" {...LINE_STROKE} stroke-width="1.2" />
      <path d="m2.4 10.4 3.2-3.8 2.8 2.4 3-4.6 2.6 2.4" {...LINE_STROKE} stroke-width="1.6" />
    </Icon>
  );
}
