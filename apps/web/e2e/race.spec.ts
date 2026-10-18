import { expect, test } from "@playwright/test";

// The whole chain in a real browser, on each track: glTF and sidecar load,
// arrow key, keyboard mapping, wasm sim on the track's colliders, transforms
// back out, Instrument panel readouts. Brands Patch is the default; the oval
// fixture is selected by query string.
for (const [name, url] of [
  ["Brands Patch", "/"],
  ["the oval", "/?track=oval"],
] as const) {
  test(`holding the up arrow drives the kart along ${name}`, async ({ page }) => {
    await page.goto(url);
    const speed = page.locator('[data-instrument="speed"]');
    const surface = page.locator('[data-instrument="surface"]');
    await expect(speed).toHaveText("0.0 m/s");
    await expect(surface).toHaveText("asphalt");

    await page.keyboard.down("ArrowUp");
    await expect
      .poll(async () => Number.parseFloat((await speed.textContent()) ?? "0"), { timeout: 5_000 })
      .toBeGreaterThan(5);
    await page.keyboard.up("ArrowUp");
    // Still on the asphalt after driving up the straight.
    await expect(surface).toHaveText("asphalt");
  });
}
