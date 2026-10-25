import { readFile } from "node:fs/promises";
import { expect, type Page, test } from "@playwright/test";
import type { ReplayFile } from "../src/replay";
import { startRace } from "./scenes";

const TRACK = "oval";
/** Long enough for a headless runner that delivers only a few frames a second. */
const POLL_TIMEOUT_MS = 20_000;

// The documented way to record a fixture: drive, press R, and the browser
// downloads the session as a replay with the hashes the wasm sim reported.
// The file is the sim's format, so dropping it into assets/replays is the
// whole step; the native half of the gate then holds the sim to it.
//
// The sim ticks only as fast as frames arrive, so wall-clock time says
// nothing about sim progress. Each phase waits on observable state instead:
// the countdown readout, the speed readout, or the recording itself, since
// every R press downloads the session so far.
test("pressing R downloads the session as a replay fixture", async ({ page }) => {
  // The recording starts with the countdown; a press during it would stall
  // the kart, so the drive waits for Go.
  await startRace(page, `/?track=${TRACK}`);
  const speed = page.locator('[data-instrument="speed"]');
  await expect(speed).toHaveText("0 km/h");

  await page.keyboard.down("ArrowUp");
  await expect
    .poll(async () => Number.parseFloat((await speed.textContent()) ?? "0"), {
      timeout: POLL_TIMEOUT_MS,
    })
    .toBeGreaterThan(18);
  await page.keyboard.down("ArrowLeft");
  await downloadReplayUntil(page, (replay) => replay.segments.some((s) => steer(s) === -1));
  await page.keyboard.up("ArrowLeft");
  const replay = await downloadReplayUntil(
    page,
    (replay) => replay.hashes.length >= 2 && steer(replay.segments.at(-1)) === 0,
  );
  await page.keyboard.up("ArrowUp");

  expect(replay.version).toBe(1);
  expect(replay.track).toBe(TRACK);
  // Seven bots fill the grid; the segments carry the player's input only.
  expect(replay.bots).toBe(7);
  // Idle through the countdown, then straight, steering, straight again:
  // consecutive ticks with the same inputs share a segment.
  expect(replay.segments.length).toBeGreaterThanOrEqual(4);
  expect(replay.segments[0]?.inputs[0]).toMatchObject({ steer: 0, throttle: 0 });
  expect(replay.segments.every((segment) => segment.inputs.length === 1)).toBe(true);
  expect(replay.segments.at(-2)?.inputs[0]).toMatchObject({ steer: -1, throttle: 1 });
  expect(replay.segments.at(-1)?.inputs[0]).toMatchObject({ steer: 0, throttle: 1 });
  expect(replay.hashes.length).toBeGreaterThanOrEqual(2);
  expect(replay.hashes.every((hash) => /^[0-9a-f]{16}$/.test(hash))).toBe(true);
});

/** Press R until the downloaded replay satisfies `done`, and return that replay. */
async function downloadReplayUntil(
  page: Page,
  done: (replay: ReplayFile) => boolean,
): Promise<ReplayFile> {
  const downloads: ReplayFile[] = [];
  await expect
    .poll(
      async () => {
        const replay = await downloadReplay(page);
        downloads.push(replay);
        return done(replay);
      },
      { timeout: POLL_TIMEOUT_MS },
    )
    .toBe(true);
  const last = downloads.at(-1);
  if (last === undefined) {
    throw new Error("the poll passed without a download");
  }
  return last;
}

async function downloadReplay(page: Page): Promise<ReplayFile> {
  const downloading = page.waitForEvent("download");
  await page.keyboard.press("r");
  const download = await downloading;
  expect(download.suggestedFilename()).toBe(`${TRACK}.replay.json`);
  return JSON.parse(await readFile(await download.path(), "utf8")) as ReplayFile;
}

function steer(segment: ReplayFile["segments"][number] | undefined): number | undefined {
  return segment?.inputs[0]?.steer;
}
