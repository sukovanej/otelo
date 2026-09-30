import Icon, { type IconProps } from "../icon";

export default function JavaIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M6.2 1.8c-1 1 .9 1.8 0 3.1M8.8 2.3c-1 1 .9 1.8 0 3.1"
        fill="none"
        stroke="#e76f00"
        stroke-width="1.1"
        stroke-linecap="round"
      />
      <path d="M2.8 7h8.4v2.3c0 2-1.6 3.6-3.6 3.6H6.4c-2 0-3.6-1.6-3.6-3.6Z" fill="#5382a1" />
      <path
        d="M11.2 7.9h.9a1.55 1.55 0 0 1 0 3.1h-1.4M2.3 14.4h9.8"
        fill="none"
        stroke="#5382a1"
        stroke-width="1.1"
        stroke-linecap="round"
      />
    </Icon>
  );
}
