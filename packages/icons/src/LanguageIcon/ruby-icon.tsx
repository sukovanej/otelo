import { WHITE } from "../colors";
import Icon, { type IconProps } from "../icon";

export default function RubyIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4.4 2.6h7.2l2.8 3.7L8 14 1.6 6.3Z" fill="#cc342d" />
      <path
        d="M1.6 6.3h12.8M4.4 2.6l1.5 3.7L8 2.6l2.1 3.7 1.5-3.7M5.9 6.3 8 14l2.1-7.7"
        fill="none"
        stroke={WHITE}
        stroke-opacity="0.45"
        stroke-width="0.7"
        stroke-linejoin="round"
      />
    </Icon>
  );
}
