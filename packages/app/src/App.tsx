import { A, type RouteSectionProps } from "@solidjs/router";

/** The frame of every page: the name and the sections. */
export default function App(props: RouteSectionProps) {
  return (
    <>
      <header class="sticky top-0 z-30 flex h-11 items-center gap-6 border-b border-line bg-surface px-4">
        <A href="/" class="font-mono text-[15px] font-semibold text-ink">
          siner
        </A>
        <nav class="flex gap-4">
          <A href="/logs" activeClass="text-ink" inactiveClass="text-muted hover:text-ink">
            Logs
          </A>
        </nav>
      </header>
      <main>{props.children}</main>
    </>
  );
}
