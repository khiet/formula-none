import { expect, test } from "@playwright/test";

// The page is plain HTTP here, so the client goes straight to the WebSocket
// fallback; the dev server proxies it to the Rust server, which answers the
// Ping. WebTransport needs TLS, which the dev server lacks; the deploy
// smoke test (deploy/smoke.sh) covers it.
test("the Instrument panel reports a round trip to the server", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Race" }).click();
  const connection = page.locator('[data-instrument="connection"]');
  await expect(connection).toHaveText(/^websocket \d+ ms$/, { timeout: 10_000 });
});
