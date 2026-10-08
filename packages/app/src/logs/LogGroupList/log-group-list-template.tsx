import { For, Show } from "solid-js";

interface LogGroupListTemplateProps {
  readonly text: string;
}

export default function LogGroupListTemplate(props: LogGroupListTemplateProps) {
  const parts = () => props.text.split(/(<(?:num|str|uuid|hex)>)/);
  return (
    <For each={parts()}>
      {(part, index) => (
        <Show when={index() % 2 === 1} fallback={part}>
          <span class="text-placeholder">{part}</span>
        </Show>
      )}
    </For>
  );
}
