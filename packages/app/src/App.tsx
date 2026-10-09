import type { RouteSectionProps } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { createSignal, For, Show, useContext } from "solid-js";

import {
  DashboardsIcon,
  LogoIcon,
  LogsIcon,
  MetricsIcon,
  ServicesIcon,
  TracesIcon,
} from "@otelo/icons";
import { Button } from "@otelo/ui";

import { ApiContext } from "./api";
import { describeError } from "./fetch";
import IndexingProgressBar from "./IndexingProgressBar";
import { LoginContext } from "./login";

const SECTIONS = [
  { href: "/services", label: "Services", icon: ServicesIcon },
  { href: "/logs", label: "Logs", icon: LogsIcon },
  { href: "/traces", label: "Traces", icon: TracesIcon },
  { href: "/metrics", label: "Metrics", icon: MetricsIcon },
  { href: "/dashboards", label: "Dashboards", icon: DashboardsIcon },
] as const;

export default function App(props: RouteSectionProps) {
  const api = useContext(ApiContext);
  const login = useContext(LoginContext);
  const queryClient = useQueryClient();
  const [loggingOut, setLoggingOut] = createSignal(false);
  const [logOutError, setLogOutError] = createSignal<string>();
  const logOutOfDaemon = async () => {
    setLoggingOut(true);
    setLogOutError(undefined);
    try {
      await api.logOut();
      queryClient.clear();
      login.askForLogin();
    } catch (error) {
      setLogOutError(describeError(error));
    } finally {
      setLoggingOut(false);
    }
  };

  return (
    <div class="flex h-dvh flex-col">
      <header class="flex h-12 shrink-0 items-center gap-4 border-b border-line bg-surface px-4 md:gap-6">
        <a href="/" class="flex items-center gap-2 font-mono text-[17px] font-semibold text-ink">
          <LogoIcon size={24} />
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
        <div class="ml-auto flex min-w-0 items-center gap-3">
          <Show when={logOutError()}>
            {(message) => (
              <span role="alert" class="truncate text-xs text-error">
                {message()}
              </span>
            )}
          </Show>
          <Button
            variant="ghost"
            size="sm"
            disabled={loggingOut()}
            onClick={() => void logOutOfDaemon()}
          >
            {loggingOut() ? "Logging out…" : "Log out"}
          </Button>
        </div>
      </header>
      <IndexingProgressBar />
      <main class="flex min-h-0 flex-1 flex-col bg-page">{props.children}</main>
    </div>
  );
}
