import { expect, test } from "@playwright/test";

// The page is plain HTTP here, so the client goes straight to the WebSocket
// fallback; the dev server proxies it to the Rust server, which answers the
// Ping. WebTransport itself needs the deployed TLS and is held by the
// deploy smoke test (deploy/smoke.sh).
test("the Instrument panel reports a round trip to the server", async ({ page }) => {
  await page.goto("/");
  const connection = page.locator('[data-instrument="connection"]');
  await expect(connection).toHaveText(/^websocket \d+ ms$/, { timeout: 10_000 });
});
