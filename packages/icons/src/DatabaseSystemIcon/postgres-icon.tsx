import { WHITE } from "../colors";
import Icon, { type IconProps } from "../icon";

export default function PostgresIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M8 2.6C6.6 1.3 3.9 1.1 2.4 2.4.9 3.8 1.1 6.6 2 8.9c.7 1.7 1.9 2.4 3.2 2l1.1-.5c.1 1.8.3 3.2 1.3 4 .8.6 2 .4 2.6-.3-.8-.1-1.4-.7-1.6-1.7l-.1-1.8c1.5.5 3.9.7 5.1-1 1.2-1.8 1.5-5 .1-6.8C12.4 1.2 9.6 1.3 8 2.6Z"
        fill="#336791"
        // The outline stands apart from the page: dark on a light one, light
        // on a dark one, as the page's `color-scheme` says.
        style={{ stroke: "light-dark(#0d1b2a, #9fb3c8)" }}
        stroke-width="0.8"
        stroke-linejoin="round"
      />
      <path
        d="M7.4 3.2C5.7 4.6 5.3 7.2 6.1 10.3M8.8 3.2c1.9 1.2 2.6 3.9 1.8 7.2"
        fill="none"
        stroke={WHITE}
        stroke-width="0.75"
        stroke-linecap="round"
      />
      <circle cx="7.3" cy="6.5" r="0.55" fill={WHITE} />
      <circle cx="9.6" cy="6.5" r="0.55" fill={WHITE} />
    </Icon>
  );
}
