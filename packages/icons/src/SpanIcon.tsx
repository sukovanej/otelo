import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function SpanIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M2.5 4.5v7M13.5 4.5v7" {...LINE_STROKE} stroke-width="1.2" />
      <rect x="4.6" y="6.4" width="6.8" height="3.2" rx="1.1" fill="currentColor" />
    </Icon>
  );
}
