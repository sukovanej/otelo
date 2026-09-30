import { WHITE } from "../colors";
import Icon, { type IconProps } from "../icon";

export default function CassandraIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M4 4.6 3 3.2M8 3.6V1.9M12 4.6l1-1.4"
        fill="none"
        stroke="#1287b1"
        stroke-width="1.1"
        stroke-linecap="round"
      />
      <path d="M1.2 9c2.6-4.4 11-4.4 13.6 0-2.6 4.4-11 4.4-13.6 0Z" fill="#1287b1" />
      <circle cx="8" cy="9" r="2.7" fill={WHITE} />
      <circle cx="8" cy="9" r="1.5" fill="#1287b1" />
      <circle cx="8.6" cy="8.4" r="0.45" fill={WHITE} />
    </Icon>
  );
}
