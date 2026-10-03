import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function MinusIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3.5 8h9" {...LINE_STROKE} />
    </Icon>
  );
}
