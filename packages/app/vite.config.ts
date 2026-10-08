import solid from "@solidjs/vite-plugin";
import tailwindcss from "@tailwindcss/vite";
import { playwright } from "@vitest/browser-playwright";
import { defineConfig } from "vitest/config";

import { pointerCommands } from "@otelo/testing/commands";

// `mise run web:dev` serves the UI here and sends the API to the daemon that
// `otelo serve` runs on its default address.
export default defineConfig({
  plugins: [solid(), tailwindcss()],
  server: {
    port: 5173,
    strictPort: true,
    proxy: { "/api": "http://127.0.0.1:7070" },
  },
  build: {
    target: "es2022",
  },
  // The .ts tests cover modules without the DOM. The .tsx tests render
  // components in Chromium with the dev build of Solid, which reports reactivity mistakes.
  test: {
    projects: [
      {
        extends: true,
        test: { name: "modules", environment: "node", include: ["tests/**/*.test.ts"] },
      },
      {
        extends: true,
        test: {
          name: "components",
          include: ["tests/**/*.test.tsx"],
          setupFiles: ["tests/setup.ts"],
          browser: {
            enabled: true,
            headless: true,
            provider: playwright(),
            instances: [{ browser: "chromium" }],
            viewport: { width: 1280, height: 800 },
            commands: pointerCommands,
          },
        },
      },
    ],
  },
});
