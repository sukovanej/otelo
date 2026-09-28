import { defineConfig } from "vitest/config";

// The tests cover modules without the DOM.
export default defineConfig({
  test: { environment: "node" },
});
