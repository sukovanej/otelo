import Icon, { type IconProps } from "./icon";

export default function GripIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g fill="currentColor">
        <circle cx="6" cy="4" r="1.1" />
        <circle cx="10" cy="4" r="1.1" />
        <circle cx="6" cy="8" r="1.1" />
        <circle cx="10" cy="8" r="1.1" />
        <circle cx="6" cy="12" r="1.1" />
        <circle cx="10" cy="12" r="1.1" />
      </g>
    </Icon>
  );
}
