import { createUniqueId } from "solid-js";

import Icon, { type IconProps } from "../icon";

export default function SqliteIcon(props: IconProps) {
  // Every icon on the page needs its own gradient id.
  const gradientId = createUniqueId();
  return (
    <Icon {...props}>
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#97d9f6" />
          <stop offset="1" stop-color="#0f80cc" />
        </linearGradient>
      </defs>
      <rect x="1" y="3.6" width="9.6" height="10.6" rx="1.8" fill={`url(#${gradientId})`} />
      {/* A dark page takes the quill in a muted outline, where navy alone
          would vanish past the square. */}
      <path
        d="M14.8 1C11.5 2 8.6 5.3 7.6 9.6c-.4 1.5-.5 3-.2 4.3.6-1.5 1.4-2.9 2.6-4.1C12.7 7.1 14.5 4.3 14.8 1Z"
        fill="#003b57"
        stroke-width="0.6"
        stroke-linejoin="round"
        style={{ stroke: "light-dark(transparent, #9fb3c8)" }}
      />
      <path
        d="M7.5 13.3 7 15.3"
        stroke-width="0.9"
        stroke-linecap="round"
        style={{ stroke: "light-dark(#003b57, #9fb3c8)" }}
      />
    </Icon>
  );
}
