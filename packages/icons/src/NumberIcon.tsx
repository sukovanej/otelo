import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function NumberIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M6.4 2.6 5 13.4M11 2.6 9.6 13.4M3 5.9h10.4M2.6 10.1H13" {...LINE_STROKE} />
    </Icon>
  );
}
