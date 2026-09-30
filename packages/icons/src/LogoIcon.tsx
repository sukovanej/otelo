import Icon, { type IconProps } from "./icon";

export default function LogoIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="1.5" y="2.5" width="13" height="2.4" rx="1.2" fill="currentColor" />
      <rect x="1.5" y="6.8" width="8.5" height="2.4" rx="1.2" fill="currentColor" opacity="0.7" />
      <rect x="1.5" y="11.1" width="11" height="2.4" rx="1.2" fill="currentColor" opacity="0.45" />
    </Icon>
  );
}
