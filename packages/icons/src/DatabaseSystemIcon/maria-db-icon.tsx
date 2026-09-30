import Icon, { type IconProps } from "../icon";

export default function MariaDbIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M1.5 13.8c0-2.8 2.5-4.4 5.5-4.8 1-3 2.5-5 4.6-5 1.8 0 3 1 3 2.2 0 1-.8 1.6-1.8 1.6-.8 0-1.2.6-1.4 1.6-.4 2.2-1.4 4-3 4.4Z"
        fill="#c0765a"
      />
      <path d="M8.6 12.2l2.8 2.2-3.6-.2Z" fill="#c0765a" />
      <circle cx="12" cy="5.5" r="0.55" fill="#003545" />
      <circle cx="14.2" cy="6.1" r="0.45" fill="#003545" />
      <path
        d="M13.5 7l1.8.5M13.3 7.4l1.5 1"
        fill="none"
        stroke="#003545"
        stroke-width="0.4"
        stroke-linecap="round"
      />
    </Icon>
  );
}
