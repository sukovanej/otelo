import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function TrashIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M2.8 4.3h10.4M6.3 4.3V2.8h3.4v1.5M4.2 4.3l.6 8.6a1 1 0 0 0 1 .9h4.4a1 1 0 0 0 1-.9l.6-8.6M6.8 7v4.2M9.2 7v4.2"
        {...LINE_STROKE}
        stroke-width="1.2"
      />
    </Icon>
  );
}
