import { defineConfig } from "vitest/config";

export default defineConfig({
  // In development the API, including the WebSocket fallback, is a locally
  // run server (`cargo run -p server`); in production Caddy proxies the same path.
  server: {
    proxy: {
      "/api": { target: "http://localhost:8080", ws: true },
    },
  },
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
