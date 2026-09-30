import Icon, { type IconProps } from "../icon";

export default function ClickHouseIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="1" y="1" width="14" height="14" rx="2.5" fill="#1c1c1c" />
      <g fill="#faff69">
        <rect x="3" y="3.5" width="1.6" height="9" rx="0.3" />
        <rect x="5.5" y="3.5" width="1.6" height="9" rx="0.3" />
        <rect x="8" y="3.5" width="1.6" height="9" rx="0.3" />
        <rect x="10.5" y="3.5" width="1.6" height="9" rx="0.3" />
        <rect x="13" y="6.7" width="1" height="2.6" rx="0.3" />
      </g>
    </Icon>
  );
}
