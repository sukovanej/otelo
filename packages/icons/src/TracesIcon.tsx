import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function TracesIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M2.3 3.2h11.4M2.3 6.4h4.2M6.5 9.6h4M10.5 12.8h3.2"
        {...LINE_STROKE}
        stroke-width="1.6"
      />
    </Icon>
  );
}
