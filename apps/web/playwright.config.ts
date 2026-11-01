import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  // Every spec waits through the race's countdown and then on sim time, and
  // a CI runner steps the sim far slower than real time.
  timeout: 120_000,
  // Software rendering already takes the whole runner, so two browsers at once
  // starve each other's sim past these timeouts. Playwright's default is half
  // the CPUs, which is two on a public repository's four-CPU runner.
  workers: process.env.CI ? 1 : "50%",
  // The dev server and, behind its /api proxy, the real Rust server: the
  // connection specs reach it over the WebSocket fallback (ADR 0011).
  webServer: [
    {
      command: "cargo run -p server",
      cwd: "../..",
      env: { LISTEN_ADDR: "127.0.0.1:8080" },
      url: "http://127.0.0.1:8080/api/health",
      reuseExistingServer: !process.env.CI,
      timeout: 600_000,
    },
    {
      command: "pnpm dev --port 5173 --strictPort",
      url: "http://localhost:5173",
      reuseExistingServer: !process.env.CI,
    },
  ],
  use: {
    baseURL: "http://localhost:5173",
    // Muted so a run does not play through the speakers beside other work;
    // the app still runs its audio, only the browser's output is silenced.
    // Playwright mutes headless Chromium on its own, so the flag matters for
    // headed specs, and opting back in must also drop Playwright's copy.
    launchOptions:
      process.env.E2E_SOUND === "1"
        ? { ignoreDefaultArgs: ["--mute-audio"] }
        : { args: ["--mute-audio"] },
  },
});
