import { A, type RouteSectionProps } from "@solidjs/router";

import { LogoIcon } from "@siner/icons";

/** The frame of every page: the name and the sections above, and the page
 * filling the rest of the window. A page scrolls its own parts. */
export default function App(props: RouteSectionProps) {
  return (
    <div class="flex h-dvh flex-col">
      <header class="flex h-12 shrink-0 items-center gap-6 border-b border-line bg-surface px-4">
        <A href="/" class="flex items-center gap-2 font-mono text-[17px] font-semibold text-ink">
          <LogoIcon size={20} class="text-accent" />
          siner
        </A>
        <nav class="flex gap-4">
          <A href="/services" activeClass="text-ink" inactiveClass="text-muted hover:text-ink">
            Services
          </A>
          <A href="/logs" activeClass="text-ink" inactiveClass="text-muted hover:text-ink">
            Logs
          </A>
          <A href="/traces" activeClass="text-ink" inactiveClass="text-muted hover:text-ink">
            Traces
          </A>
        </nav>
      </header>
      <main class="flex min-h-0 flex-1 flex-col bg-page">{props.children}</main>
    </div>
  );
}
