import { readFile } from "node:fs/promises";
import { expect, type Page, test } from "@playwright/test";
import type { ReplayFile } from "../src/replay";

// The documented way to record a fixture: drive, press R, and the browser
// downloads the session as a replay with the hashes the wasm sim reported.
// The file is the sim's format, so dropping it into assets/replays is the
// whole step; the native half of the gate then holds the sim to it.
//
// The sim ticks only as fast as frames arrive, and a headless CI runner
// delivers them slowly, so wall-clock time says nothing about sim progress.
// Each phase waits on the recording itself: every R press downloads the
// session so far.
test("pressing R downloads the session as a replay fixture", async ({ page }) => {
  await page.goto("/?track=oval");
  await expect(page.locator('[data-instrument="speed"]')).toHaveText("0.0 m/s");

  await page.keyboard.down("ArrowUp");
  await page.keyboard.down("ArrowLeft");
  await expect
    .poll(async () => (await pressR(page)).segments.some((segment) => steer(segment) === -1))
    .toBe(true);
  await page.keyboard.up("ArrowLeft");
  let replay = await pressR(page);
  await expect
    .poll(
      async () => {
        replay = await pressR(page);
        return replay.hashes.length >= 2 && steer(replay.segments.at(-1)) === 0;
      },
      { timeout: 20_000 },
    )
    .toBe(true);
  await page.keyboard.up("ArrowUp");

  expect(replay.version).toBe(1);
  expect(replay.track).toBe("oval");
  // Steering, then straight: consecutive ticks with the same inputs share a segment.
  expect(replay.segments.length).toBeGreaterThanOrEqual(2);
  expect(replay.segments.every((segment) => segment.inputs.length === 1)).toBe(true);
  expect(replay.segments.at(-2)?.inputs[0]).toMatchObject({ steer: -1, throttle: 1 });
  expect(replay.segments.at(-1)?.inputs[0]).toMatchObject({ steer: 0, throttle: 1 });
  expect(replay.hashes.length).toBeGreaterThanOrEqual(2);
  expect(replay.hashes.every((hash) => /^[0-9a-f]{16}$/.test(hash))).toBe(true);
});

async function pressR(page: Page): Promise<ReplayFile> {
  const downloading = page.waitForEvent("download");
  await page.keyboard.press("r");
  const download = await downloading;
  expect(download.suggestedFilename()).toBe("oval.replay.json");
  return JSON.parse(await readFile(await download.path(), "utf8")) as ReplayFile;
}

function steer(segment: ReplayFile["segments"][number] | undefined): number | undefined {
  return segment?.inputs[0]?.steer;
}
