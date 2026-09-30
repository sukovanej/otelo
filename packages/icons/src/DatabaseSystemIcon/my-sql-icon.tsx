import Icon, { type IconProps } from "../icon";

export default function MySqlIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M1.8 4.6C2.8 3 4.8 1.9 7 1.9c1.4 0 2.5.7 3.2 1.9l3.5-1.3-2.6 2.9c.5 2 .5 4.3-.1 6.6l3.2 1.9-3.3-.5-1.5 1.7.5-2.7c.2-2.2-.4-4.4-2-6C6.8 5.3 4.4 4.9 1.8 4.6Z"
        fill="#00758f"
        fill-opacity="0.18"
        stroke="#00758f"
        stroke-width="1.2"
        stroke-linejoin="round"
      />
      <path
        d="M8.5 7.4 6.9 9.2"
        fill="none"
        stroke="#00758f"
        stroke-width="1.2"
        stroke-linecap="round"
      />
      <circle cx="5.3" cy="3.4" r="0.6" fill="#00758f" />
    </Icon>
  );
}
