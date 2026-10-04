import { createEffect } from "solid-js";

const SHOW_MORE_AHEAD_PX = 600;

interface EndWatch {
  readonly scrollElement: HTMLElement;
  readonly canShowMore: boolean;
}

interface ListContentEndProps {
  readonly scrollElement: HTMLElement;
  readonly canShowMore: boolean;
  readonly loading: boolean;
  readonly onShowMore: () => void;
}

export default function ListContentEnd(props: ListContentEndProps) {
  let endElement!: HTMLDivElement;

  createEffect(
    (): EndWatch => ({
      scrollElement: props.scrollElement,
      canShowMore: props.canShowMore && !props.loading,
    }),
    (watch) => {
      const observer = new IntersectionObserver(
        (entries) => {
          if (watch.canShowMore && entries.at(-1)?.isIntersecting) props.onShowMore();
        },
        { root: watch.scrollElement, rootMargin: `0px 0px ${SHOW_MORE_AHEAD_PX}px 0px` },
      );
      observer.observe(endElement);
      return () => observer.disconnect();
    },
  );

  return <div ref={endElement} aria-hidden="true" />;
}
