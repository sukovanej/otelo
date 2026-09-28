import tailwindcss from "@tailwindcss/vite";
import solid from "vite-plugin-solid";
import { defineConfig } from "vitest/config";

// `mise run web:dev` serves the UI here and sends the API to the daemon that
// `siner serve` runs on its default address.
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
  // The tests cover modules without the DOM.
  test: {
    environment: "node",
  },
});
