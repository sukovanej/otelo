import { INK } from "../colors";
import Icon, { type IconProps } from "../icon";

export default function JavaScriptIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="1" y="1" width="14" height="14" rx="2" fill="#f7df1e" />
      <path
        d="M7.3 7.2v4.3c0 1-.6 1.6-1.5 1.6-.7 0-1.2-.4-1.4-1M12.7 7.9c-.3-.5-.8-.8-1.5-.8-.8 0-1.3.4-1.3 1s.5.9 1.4 1.2c1 .3 1.6.7 1.6 1.6s-.7 1.6-1.8 1.6c-.9 0-1.6-.4-1.9-1.1"
        fill="none"
        stroke={INK}
        stroke-width="1.25"
        stroke-linecap="round"
      />
    </Icon>
  );
}
