import { expect, test } from "@playwright/test";

// The whole chain in a real browser: arrow key, keyboard mapping, wasm sim,
// transforms back out, Instrument panel readout.
test("holding the up arrow drives the kart", async ({ page }) => {
  await page.goto("/");
  const speed = page.locator('[data-instrument="speed"]');
  await expect(speed).toHaveText("0.0 m/s");

  await page.keyboard.down("ArrowUp");
  await expect
    .poll(async () => Number.parseFloat((await speed.textContent()) ?? "0"), { timeout: 5_000 })
    .toBeGreaterThan(5);
  await page.keyboard.up("ArrowUp");
});
