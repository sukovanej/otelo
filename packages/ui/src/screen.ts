import { type Accessor, createSignal, onCleanup } from "solid-js";

const NARROW_SCREEN_QUERY = "(width < 48rem)";

export function createNarrowScreenSignal(): Accessor<boolean> {
  const narrowScreen = window.matchMedia(NARROW_SCREEN_QUERY);
  const [isNarrowScreen, setIsNarrowScreen] = createSignal(narrowScreen.matches);
  const followNarrowScreen = (e: MediaQueryListEvent) => setIsNarrowScreen(e.matches);
  narrowScreen.addEventListener("change", followNarrowScreen);
  onCleanup(() => narrowScreen.removeEventListener("change", followNarrowScreen));
  return isNarrowScreen;
}
