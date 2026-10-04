import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function AreaChartIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="m1.8 9.4 3.6-4 3.2 2.6 3.2-4.4 2.4 2v8.2H1.8Z" fill="currentColor" opacity="0.3" />
      <path d="m1.8 9.4 3.6-4 3.2 2.6 3.2-4.4 2.4 2" {...LINE_STROKE} stroke-width="1.6" />
    </Icon>
  );
}
