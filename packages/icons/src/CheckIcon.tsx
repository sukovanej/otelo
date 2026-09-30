import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function CheckIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3.5 8.5 6.5 11.5 12.5 4.5" {...LINE_STROKE} stroke-width="1.6" />
    </Icon>
  );
}
