import { defineConfig } from "vitest/config";

// The icons are drawings, with nothing to test without the DOM.
export default defineConfig({
  test: { environment: "node", passWithNoTests: true },
});
