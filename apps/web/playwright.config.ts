import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
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
  use: { baseURL: "http://localhost:5173" },
});
