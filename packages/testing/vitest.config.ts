import { defineConfig } from "vitest/config";

// The helpers run inside the tests of the other packages.
export default defineConfig({
  test: { environment: "node", passWithNoTests: true },
});
