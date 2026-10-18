import { expect, test } from "@playwright/test";

// The whole chain in a real browser, on each track: glTF and sidecar load,
// a key, keyboard mapping, wasm sim on the track's colliders, transforms
// back out, Instrument panel readouts. Brands Patch is the default; the oval
// fixture is selected by query string. The arrows drive one track and WASD
// the other, so both layouts are covered.
for (const [name, url, throttleKey] of [
  ["Brands Patch", "/", "ArrowUp"],
  ["the oval", "/?track=oval", "w"],
] as const) {
  test(`holding ${throttleKey} drives the kart along ${name}`, async ({ page }) => {
    await page.goto(url);
    const speed = page.locator('[data-instrument="speed"]');
    const surface = page.locator('[data-instrument="surface"]');
    await expect(speed).toHaveText("0.0 m/s");
    await expect(surface).toHaveText("asphalt");

    await page.keyboard.down(throttleKey);
    await expect
      .poll(async () => Number.parseFloat((await speed.textContent()) ?? "0"), { timeout: 5_000 })
      .toBeGreaterThan(5);
    await page.keyboard.up(throttleKey);
    // Still on the asphalt after driving up the straight.
    await expect(surface).toHaveText("asphalt");
  });
}

test("holding the brake at a standstill backs the kart up", async ({ page }) => {
  await page.goto("/?track=oval");
  const speed = page.locator('[data-instrument="speed"]');
  await expect(speed).toHaveText("0.0 m/s");
  await page.keyboard.down("ArrowDown");
  await expect
    .poll(async () => Number.parseFloat((await speed.textContent()) ?? "0"), { timeout: 5_000 })
    .toBeLessThan(-2);
  await page.keyboard.up("ArrowDown");
});
