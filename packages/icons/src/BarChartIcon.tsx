import Icon, { type IconProps } from "./icon";

export default function BarChartIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="2" y="8" width="2.8" height="6" rx="0.6" fill="currentColor" />
      <rect x="6.6" y="3" width="2.8" height="11" rx="0.6" fill="currentColor" />
      <rect x="11.2" y="6" width="2.8" height="8" rx="0.6" fill="currentColor" />
    </Icon>
  );
}
