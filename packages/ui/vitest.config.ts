import solid from "@solidjs/vite-plugin";
import tailwindcss from "@tailwindcss/vite";
import { playwright } from "@vitest/browser-playwright";
import { defineConfig } from "vitest/config";

import { pointerCommands } from "@otelo/testing/commands";

// The .ts tests cover modules without the DOM. The .tsx tests render
// components in Chromium with the dev build of Solid, which reports reactivity mistakes.
export default defineConfig({
  plugins: [solid(), tailwindcss()],
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
