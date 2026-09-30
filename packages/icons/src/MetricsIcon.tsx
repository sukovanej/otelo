import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function MetricsIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M2.3 2.6v10.9a.2.2 0 0 0 .2.2h11.2" {...LINE_STROKE} stroke-width="1.2" />
      <path d="m4.9 10.3 2.6-3.2 2.3 2 3.5-4.6" {...LINE_STROKE} stroke-width="1.6" />
    </Icon>
  );
}
