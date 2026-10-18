import { expect, test } from "@playwright/test";

// The whole chain in a real browser, on each track: glTF and sidecar load,
// a key, keyboard mapping, wasm sim on the track's colliders with seven
// bots on the grid behind the player, transforms back out, Instrument panel
// readouts. Brands Patch is the default; the oval fixture is selected by
// query string. The arrows drive one track and WASD the other, so both
// layouts are covered.
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
    // The Instrument panel's lap readouts come from the sim: on lap one, the
    // lap clock running and no best lap yet, and last once the seven bots
    // behind on the grid have driven past the idle kart.
    await expect(page.locator('[data-instrument="lap"]')).toHaveText("Lap 1/3");
    await expect(page.locator('[data-instrument="position"]')).toHaveText("8th", {
      timeout: 30_000,
    });
    await expect(page.locator('[data-instrument="best-lap"]')).toHaveText("Best --:--.---");
    await expect(page.locator('[data-instrument="lap-time"]')).not.toHaveText("0:00.000");

    await page.keyboard.down(throttleKey);
    await expect
      .poll(async () => Number.parseFloat((await speed.textContent()) ?? "0"), { timeout: 5_000 })
      .toBeGreaterThan(5);
    await page.keyboard.up(throttleKey);
    // Still on the asphalt after driving up the straight.
    await expect(surface).toHaveText("asphalt");
  });
}

test("backing up the Brands Patch grid for two seconds shows the wrong-way warning until the kart drives on", async ({
  page,
}) => {
  // The warning needs two seconds of sim time and clearing most of another.
  // A CI runner steps the sim far slower than real time, so the waits are
  // sized in wall time with room to spare.
  test.setTimeout(120_000);
  await page.goto("/");
  const speed = page.locator('[data-instrument="speed"]');
  const wrongWay = page.locator('[data-instrument="wrong-way"]');
  await expect(speed).toHaveText("0.0 m/s");
  await expect(wrongWay).toBeHidden();
  // Once the bots behind on the grid have all gone past, the straight
  // behind the kart is clear to back up along.
  await expect(page.locator('[data-instrument="position"]')).toHaveText("8th", {
    timeout: 30_000,
  });
  // Reversing from the grid takes the kart away from the line it faces.
  await page.keyboard.down("ArrowDown");
  await expect(wrongWay).toHaveText("WRONG WAY", { timeout: 60_000 });
  await expect(wrongWay).toBeVisible();
  await page.keyboard.up("ArrowDown");
  await page.keyboard.down("ArrowUp");
  await expect(wrongWay).toBeHidden({ timeout: 30_000 });
  await page.keyboard.up("ArrowUp");
});

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
