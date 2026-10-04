import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function NoteIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g {...LINE_STROKE} stroke-width="1.2">
        <rect x="2.8" y="2" width="10.4" height="12" rx="1.6" />
        <path d="M5.4 5.6h5.2M5.4 8h5.2M5.4 10.4h3" />
      </g>
    </Icon>
  );
}
