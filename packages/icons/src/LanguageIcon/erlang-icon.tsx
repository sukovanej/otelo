import { WHITE } from "../colors";
import Icon, { type IconProps } from "../icon";

export default function ErlangIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="1" y="1" width="14" height="14" rx="2" fill="#a90533" />
      <path
        d="M4.4 8.3h7.2c0-2.1-1.5-3.5-3.5-3.5S4.4 6.2 4.4 8.3s1.5 3.5 3.7 3.5c1.4 0 2.5-.5 3.2-1.4"
        fill="none"
        stroke={WHITE}
        stroke-width="1.4"
        stroke-linecap="round"
      />
    </Icon>
  );
}
