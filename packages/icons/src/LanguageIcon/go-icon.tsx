import Icon, { type IconProps } from "../icon";

export default function GoIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g fill="none" stroke="#00add8" stroke-width="1.4" stroke-linecap="round">
        <path d="M0.8 6.4h2.4M0.4 8.3h2.2M1 10.2h1.6" />
        <path d="M8.7 6.3a2.4 2.4 0 1 0 .4 2.3H7.5" />
        <circle cx="12.9" cy="8" r="2.35" />
      </g>
    </Icon>
  );
}
