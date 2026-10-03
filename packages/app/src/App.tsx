import type { RouteSectionProps } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { For } from "solid-js";

import { logOut } from "@otelo/api";
import { LogoIcon, LogsIcon, MetricsIcon, ServicesIcon, TracesIcon } from "@otelo/icons";
import { Button } from "@otelo/ui";

import { askForLogin } from "./login";

const SECTIONS = [
  { href: "/services", label: "Services", icon: ServicesIcon },
  { href: "/logs", label: "Logs", icon: LogsIcon },
  { href: "/traces", label: "Traces", icon: TracesIcon },
  { href: "/metrics", label: "Metrics", icon: MetricsIcon },
] as const;

export default function App(props: RouteSectionProps) {
  const queryClient = useQueryClient();
  const logOutOfDaemon = async () => {
    await logOut();
    queryClient.clear();
    askForLogin();
  };

  return (
    <div class="flex h-dvh flex-col">
      <header class="flex h-12 shrink-0 items-center gap-6 border-b border-line bg-surface px-4">
        <a href="/" class="flex items-center gap-2 font-mono text-[17px] font-semibold text-ink">
          <LogoIcon size={20} class="text-accent" />
          otelo
        </a>
        <nav class="flex gap-5">
          <For each={SECTIONS}>
            {(section) => (
              <a
                href={section.href}
                class="group flex items-center gap-1.5 text-muted hover:text-ink data-active:text-ink"
              >
                <section.icon class="group-data-active:text-accent" />
                {section.label}
              </a>
            )}
          </For>
        </nav>
        <Button variant="ghost" size="sm" class="ml-auto" onClick={() => void logOutOfDaemon()}>
          Log out
        </Button>
      </header>
      <main class="flex min-h-0 flex-1 flex-col bg-page">{props.children}</main>
    </div>
  );
}
