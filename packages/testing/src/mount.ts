import { type JSX, render } from "@solidjs/web";
import { onTestFinished } from "vitest";

// The view leaves the page when the test finishes, so the next test starts on an empty one.
export function mountView(view: () => JSX.Element): HTMLElement {
  const container = document.body.appendChild(document.createElement("div"));
  const dispose = render(view, container);
  onTestFinished(() => {
    dispose();
    container.remove();
  });
  return container;
}
