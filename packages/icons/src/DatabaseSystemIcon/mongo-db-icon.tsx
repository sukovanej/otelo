import { WHITE } from "../colors";
import Icon, { type IconProps } from "../icon";

export default function MongoDbIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M8 .9c3.6 3.5 3.9 8.6.6 12.2L8.3 15h-.6l-.3-1.9C4.1 9.5 4.4 4.4 8 .9Z"
        fill="#47a248"
      />
      <path
        d="M8 2.6v12"
        fill="none"
        stroke={WHITE}
        stroke-opacity="0.55"
        stroke-width="0.6"
        stroke-linecap="round"
      />
    </Icon>
  );
}
