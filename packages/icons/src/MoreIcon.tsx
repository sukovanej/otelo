import Icon, { type IconProps } from "./icon";

export default function MoreIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g fill="currentColor">
        <circle cx="3.5" cy="8" r="1.3" />
        <circle cx="8" cy="8" r="1.3" />
        <circle cx="12.5" cy="8" r="1.3" />
      </g>
    </Icon>
  );
}
