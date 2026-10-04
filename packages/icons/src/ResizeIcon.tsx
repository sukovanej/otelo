import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function ResizeIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M13.5 6.5 6.5 13.5M13.5 10.5l-3 3" {...LINE_STROKE} />
    </Icon>
  );
}
