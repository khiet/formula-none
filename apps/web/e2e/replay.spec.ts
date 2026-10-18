import { expect, test } from "@playwright/test";

// The documented way to record a fixture: drive, press R, and the browser
// downloads the session as a replay with the hashes the wasm sim reported.
// The file is the sim's format, so dropping it into assets/replays is the
// whole step; the native half of the gate then holds the sim to it.
test("pressing R downloads the session as a replay fixture", async ({ page }) => {
  await page.goto("/?track=oval");
  const speed = page.locator('[data-instrument="speed"]');
  await expect(speed).toHaveText("0.0 m/s");

  await page.keyboard.down("ArrowUp");
  await expect
    .poll(async () => Number.parseFloat((await speed.textContent()) ?? "0"), { timeout: 5_000 })
    .toBeGreaterThan(5);
  await page.keyboard.down("ArrowLeft");
  // Past the second hash, so the recording holds more than one.
  await page.waitForTimeout(1_500);
  await page.keyboard.up("ArrowLeft");
  await page.keyboard.up("ArrowUp");

  const downloading = page.waitForEvent("download");
  await page.keyboard.press("r");
  const download = await downloading;
  expect(download.suggestedFilename()).toBe("oval.replay.json");

  const path = await download.path();
  const { readFile } = await import("node:fs/promises");
  const replay = JSON.parse(await readFile(path, "utf8")) as {
    version: number;
    seed: number;
    track: string;
    segments: { ticks: number; inputs: { steer: number; throttle: number; brake: number }[] }[];
    hashes: string[];
  };
  expect(replay.version).toBe(1);
  expect(replay.track).toBe("oval");
  expect(replay.segments.length).toBeGreaterThanOrEqual(3);
  expect(replay.segments.every((segment) => segment.inputs.length === 1)).toBe(true);
  expect(replay.segments.some((segment) => segment.inputs[0]?.steer === -1)).toBe(true);
  expect(replay.hashes.length).toBeGreaterThanOrEqual(2);
  expect(replay.hashes.every((hash) => /^[0-9a-f]{16}$/.test(hash))).toBe(true);
});
