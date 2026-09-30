import Icon, { type IconProps } from "../icon";
import { LINE_STROKE } from "../line-stroke";

export default function CodeIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M5.2 4.6 1.8 8l3.4 3.4M10.8 4.6 14.2 8l-3.4 3.4M9.1 3.2 6.9 12.8" {...LINE_STROKE} />
    </Icon>
  );
}
