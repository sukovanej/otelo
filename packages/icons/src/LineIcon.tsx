import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function LineIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="m1.8 11.6 3.6-5 3.2 3.4 3.2-5.6 2.4 2.6" {...LINE_STROKE} stroke-width="1.6" />
    </Icon>
  );
}
