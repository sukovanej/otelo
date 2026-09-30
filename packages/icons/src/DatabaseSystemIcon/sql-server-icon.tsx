import { WHITE } from "../colors";
import Icon, { type IconProps } from "../icon";

export default function SqlServerIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="2.5" y="1.3" width="11" height="13.4" rx="2" fill="#cc2927" />
      <path
        d="M2.5 5.8h11M2.5 10.2h11"
        fill="none"
        stroke={WHITE}
        stroke-opacity="0.7"
        stroke-width="0.8"
      />
      <g fill={WHITE}>
        <circle cx="11" cy="3.6" r="0.7" />
        <circle cx="11" cy="8" r="0.7" />
        <circle cx="11" cy="12.4" r="0.7" />
      </g>
    </Icon>
  );
}
