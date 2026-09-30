import { For } from "solid-js";

interface LogGroupListTemplateProps {
  readonly text: string;
}

export default function LogGroupListTemplate(props: LogGroupListTemplateProps) {
  const parts = () => props.text.split(/(<(?:num|str|uuid|hex)>)/);
  return (
    <For each={parts()}>
      {(part, index) => (index() % 2 === 1 ? <span class="text-placeholder">{part}</span> : part)}
    </For>
  );
}
