import type { RouteSectionProps } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { For } from "solid-js";

import { logOut } from "@otelo/api";
import {
  DashboardsIcon,
  LogoIcon,
  LogsIcon,
  MetricsIcon,
  ServicesIcon,
  TracesIcon,
} from "@otelo/icons";
import { Button } from "@otelo/ui";

import { askForLogin } from "./login";

const SECTIONS = [
  { href: "/services", label: "Services", icon: ServicesIcon },
  { href: "/logs", label: "Logs", icon: LogsIcon },
  { href: "/traces", label: "Traces", icon: TracesIcon },
  { href: "/metrics", label: "Metrics", icon: MetricsIcon },
  { href: "/dashboards", label: "Dashboards", icon: DashboardsIcon },
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
      <header class="flex h-12 shrink-0 items-center gap-4 border-b border-line bg-surface px-4 md:gap-6">
        <a href="/" class="flex items-center gap-2 font-mono text-[17px] font-semibold text-ink">
          <LogoIcon size={20} class="text-accent" />
          <span class="hidden sm:inline">otelo</span>
        </a>
        <nav class="flex min-w-0 gap-4 md:gap-5">
          <For each={SECTIONS}>
            {(section) => (
              <a
                href={section.href}
                aria-label={section.label}
                title={section.label}
                class="group flex items-center gap-1.5 text-muted hover:text-ink data-active:text-ink"
              >
                <section.icon class="group-data-active:text-accent" />
                <span class="hidden md:inline">{section.label}</span>
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
