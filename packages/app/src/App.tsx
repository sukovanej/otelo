import { A, type RouteSectionProps } from "@solidjs/router";

/** The frame of every page: the name and the sections above, and the page
 * filling the rest of the window. A page scrolls its own parts. */
export default function App(props: RouteSectionProps) {
  return (
    <div class="flex h-dvh flex-col">
      <header class="flex h-11 shrink-0 items-center gap-6 border-b border-line bg-surface px-4">
        <A href="/" class="flex items-center gap-2 font-mono text-[15px] font-semibold text-ink">
          <svg width="18" height="18" viewBox="0 0 18 18" class="text-accent" aria-hidden="true">
            <rect x="2" y="3" width="14" height="2.5" rx="1.25" fill="currentColor" />
            <rect
              x="2"
              y="7.75"
              width="9"
              height="2.5"
              rx="1.25"
              fill="currentColor"
              opacity="0.7"
            />
            <rect
              x="2"
              y="12.5"
              width="12"
              height="2.5"
              rx="1.25"
              fill="currentColor"
              opacity="0.45"
            />
          </svg>
          siner
        </A>
        <nav class="flex gap-4">
          <A href="/logs" activeClass="text-ink" inactiveClass="text-muted hover:text-ink">
            Logs
          </A>
        </nav>
      </header>
      <main class="flex min-h-0 flex-1 flex-col">{props.children}</main>
    </div>
  );
}
