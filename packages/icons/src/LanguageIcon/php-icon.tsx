import { WHITE } from "../colors";
import Icon, { type IconProps } from "../icon";

export default function PhpIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <ellipse cx="8" cy="8" rx="7.4" ry="5" fill="#777bb4" />
      <path
        d="M2.9 10.8V6.2h1.4a1.1 1.1 0 0 1 0 2.2H2.9M6.4 5.4v4.1M6.4 7.6c.3-.5.7-.8 1.2-.8.6 0 .9.4.9 1v1.7M10.5 10.8V6.2h1.4a1.1 1.1 0 0 1 0 2.2h-1.4"
        fill="none"
        stroke={WHITE}
        stroke-width="1"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </Icon>
  );
}
