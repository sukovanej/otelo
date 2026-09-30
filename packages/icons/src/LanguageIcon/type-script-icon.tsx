import { WHITE } from "../colors";
import Icon, { type IconProps } from "../icon";

export default function TypeScriptIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="1" y="1" width="14" height="14" rx="2" fill="#3178c6" />
      <path
        d="M3.4 7.4h4M5.4 7.4v5.5M12.7 7.9c-.3-.5-.8-.8-1.5-.8-.8 0-1.3.4-1.3 1s.5.9 1.4 1.2c1 .3 1.6.7 1.6 1.6s-.7 1.6-1.8 1.6c-.9 0-1.6-.4-1.9-1.1"
        fill="none"
        stroke={WHITE}
        stroke-width="1.25"
        stroke-linecap="round"
      />
    </Icon>
  );
}
