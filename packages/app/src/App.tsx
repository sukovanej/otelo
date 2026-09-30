import { A, type RouteSectionProps } from "@solidjs/router";
import { For } from "solid-js";

import { LogoIcon, LogsIcon, ServicesIcon, TracesIcon } from "@otelo/icons";

const SECTIONS = [
  { href: "/services", label: "Services", icon: ServicesIcon },
  { href: "/logs", label: "Logs", icon: LogsIcon },
  { href: "/traces", label: "Traces", icon: TracesIcon },
] as const;

export default function App(props: RouteSectionProps) {
  return (
    <div class="flex h-dvh flex-col">
      <header class="flex h-12 shrink-0 items-center gap-6 border-b border-line bg-surface px-4">
        <A href="/" class="flex items-center gap-2 font-mono text-[17px] font-semibold text-ink">
          <LogoIcon size={20} class="text-accent" />
          otelo
        </A>
        <nav class="flex gap-5">
          <For each={SECTIONS}>
            {(section) => (
              <A
                href={section.href}
                class="group flex items-center gap-1.5"
                activeClass="text-ink"
                inactiveClass="text-muted hover:text-ink"
              >
                <section.icon class="group-aria-[current=page]:text-accent" />
                {section.label}
              </A>
            )}
          </For>
        </nav>
      </header>
      <main class="flex min-h-0 flex-1 flex-col bg-page">{props.children}</main>
    </div>
  );
}
