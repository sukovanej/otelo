import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function LogsIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M2.6 4h.01M2.6 8h.01M2.6 12h.01" {...LINE_STROKE} stroke-width="1.8" />
      <path d="M5.8 4h7.7M5.8 8h4.9M5.8 12h6.5" {...LINE_STROKE} />
    </Icon>
  );
}
