import { defineConfig } from "vitest/config";

// One root config so `pnpm test` runs every package's unit tests; Playwright
// specs are excluded here because they run under their own runner.
export default defineConfig({
  test: {
    projects: ["apps/*", "tools/*"],
  },
});
