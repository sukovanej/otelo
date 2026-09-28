import { createSignal, Show } from "solid-js";

import { control, cx, plain, type Size, sizes, textInput } from "./classes";
import Select from "./Select";

const PRESETS = ["5m", "15m", "1h", "6h", "24h", "7d"];

const CUSTOM = "custom";

/**
 * The range of a query: a preset duration before now, or a custom `since`
 * and `until`, each a duration before now such as `2h` or an RFC 3339
 * timestamp.
 */
export default function RangePicker(props: {
  /** Durations such as `1h`. 5m, 15m, 1h, 6h, 24h, and 7d when missing. */
  presets?: string[];
  since: string;
  until: string;
  onChange: (since: string, until: string) => void;
  size?: Size | undefined;
}) {
  const presets = () => props.presets ?? PRESETS;
  const inputClass = () =>
    cx(control, plain, sizes[props.size ?? "md"], textInput, "w-[17ch] font-mono");
  const isPreset = () => props.until === "" && presets().includes(props.since);
  const [custom, setCustom] = createSignal(!isPreset());
  let sinceInput!: HTMLInputElement;
  let untilInput!: HTMLInputElement;

  const applyCustom = () => {
    const since = sinceInput.value.trim();
    if (since !== "") props.onChange(since, untilInput.value.trim());
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter") applyCustom();
  };

  return (
    <div class="flex items-center gap-1.5">
      <Select
        label="Range"
        size={props.size}
        options={[
          ...presets().map((preset) => ({ value: preset, label: `Last ${preset}` })),
          { value: CUSTOM, label: "Custom" },
        ]}
        value={custom() || !isPreset() ? CUSTOM : props.since}
        onChange={(value) => {
          if (value === CUSTOM) {
            setCustom(true);
          } else {
            setCustom(false);
            props.onChange(value, "");
          }
        }}
      />
      <Show when={custom() || !isPreset()}>
        <input
          ref={sinceInput}
          class={inputClass()}
          aria-label="Since"
          title="Since: a duration before now, such as 2h, or an RFC 3339 time"
          placeholder="since, e.g. 2h"
          value={props.since}
          onKeyDown={onKeyDown}
          onBlur={applyCustom}
        />
        <span class="text-muted">to</span>
        <input
          ref={untilInput}
          class={inputClass()}
          aria-label="Until"
          title="Until: a duration before now or an RFC 3339 time; empty for now"
          placeholder="now"
          value={props.until}
          onKeyDown={onKeyDown}
          onBlur={applyCustom}
        />
      </Show>
    </div>
  );
}
