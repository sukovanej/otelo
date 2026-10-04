import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function PencilIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M10.3 3.2 12.8 5.7 6 12.5 3 13l.5-3Z M9 4.5l2.5 2.5"
        {...LINE_STROKE}
        stroke-width="1.2"
      />
    </Icon>
  );
}
