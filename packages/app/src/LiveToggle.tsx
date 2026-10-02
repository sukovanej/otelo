import { Checkbox } from "@otelo/ui";

import { LIVE_RELOAD_MS } from "./fetch";

interface LiveToggleProps {
  readonly live: boolean;
  readonly until: string;
  readonly onChange: (live: boolean) => void;
}

export default function LiveToggle(props: LiveToggleProps) {
  return (
    <Checkbox
      checked={props.live}
      disabled={props.until !== ""}
      onChange={(checked) => props.onChange(checked)}
      title={
        props.until === ""
          ? `Reload every ${LIVE_RELOAD_MS / 1000} s`
          : "Live needs a range that ends now"
      }
    >
      Live
    </Checkbox>
  );
}
