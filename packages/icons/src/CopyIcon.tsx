import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function CopyIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g {...LINE_STROKE} stroke-width="1.2">
        <rect x="5.5" y="5.5" width="8" height="8" rx="1.2" />
        <path d="M10.5 5.5V3.7a1.2 1.2 0 0 0-1.2-1.2H3.7a1.2 1.2 0 0 0-1.2 1.2v5.6a1.2 1.2 0 0 0 1.2 1.2h1.8" />
      </g>
    </Icon>
  );
}
