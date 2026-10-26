import { readFileSync } from "node:fs";
import { type BrowserContext, expect, type Page, test } from "@playwright/test";
import { loadBridge } from "../src/bridge";
import { autopilot, centreline } from "./autopilot";

// Eight browser tabs on one machine complete a three-lap race on greybox
// Brands Patch (issue 17): every tab is a human on the grid, so the race
// ends only when one of them finishes. The creator's tab is driven by an
// autopilot that reads the kart's place from the snapshots on the page's
// own WebSocket, decoded through the bridge, and presses the arrow keys;
// the other seven idle and are placed by progress when the cap runs
// out. Eight WebGL tabs through a whole race is a local smoke test, not a
// CI job.

test.skip(!!process.env.CI, "eight WebGL tabs through a whole race are a local smoke test");
// Headed, so the tabs render on the GPU as a player's do: in a local run,
// eight tabs on the headless software renderer got a few frames a second
// each, and inputs that late ran the autopilot's kart wide of the gates.
test.use({ headless: false });
test.setTimeout(900_000);

const WASM = new URL("../src/generated/bridge/bridge_bg.wasm", import.meta.url);

async function roomCode(page: Page): Promise<string> {
  const code = (await page.locator("[data-room-code]").textContent()) ?? "";
  expect(code).toMatch(/^[A-HJ-KM-NP-Z]{4}$/);
  return code;
}

test("eight tabs complete a three-lap race on Brands Patch", async ({ browser }) => {
  const bridge = await loadBridge(readFileSync(WASM));
  const line = centreline();
  // One tab per kart on the grid, so no bot races.
  const TABS = bridge.kartsPerRace;
  const contexts: BrowserContext[] = [];
  const pages: Page[] = [];
  for (let tab = 0; tab < TABS; tab++) {
    const context = await browser.newContext();
    contexts.push(context);
    pages.push(await context.newPage());
  }
  const creator = pages[0] as Page;
  const driver = autopilot(creator, bridge, line);

  await creator.goto("/");
  await creator.getByLabel("Nickname").fill("driver");
  await creator.getByRole("button", { name: "Create room" }).click();
  await expect(creator.locator("#room-scene")).toBeVisible({ timeout: 15_000 });
  const code = await roomCode(creator);
  for (const [index, page] of pages.slice(1).entries()) {
    await page.goto(`/?room=${code}`);
    await page.getByLabel("Nickname").fill(`idle${index + 1}`);
    await page.getByRole("button", { name: "Join room" }).click();
    await expect(page.locator("#room-scene")).toBeVisible({ timeout: 15_000 });
  }
  for (const page of pages) {
    await expect(page.locator("li[data-slot]")).toHaveCount(TABS);
    await page.getByRole("button", { name: "Ready", exact: true }).click();
  }
  await expect(creator.getByRole("button", { name: "Start" })).toBeEnabled({ timeout: 15_000 });
  await creator.getByRole("button", { name: "Start" }).click();

  for (const page of pages) {
    await expect(page.locator("#instrument-panel")).toBeVisible({ timeout: 60_000 });
  }
  // The autopilot laps; the race ends the cap after it finishes, and
  // every tab reaches the results with every kart placed.
  await expect
    .poll(() => driver.laps(), { timeout: 480_000, intervals: [2_000] })
    .toBeGreaterThanOrEqual(bridge.lapsPerRace);
  for (const page of pages) {
    await expect(page.locator("#results-scene")).toBeVisible({ timeout: 300_000 });
    await expect(page.locator("li[data-result]")).toHaveCount(TABS);
  }
  await expect(creator.locator("li[data-result='1st']")).toContainText("You");
  await expect(creator.locator("li[data-result='1st']")).not.toContainText("(not finished)");
  await expect(creator.locator(`li[data-result='${TABS}th']`)).toContainText("(not finished)");

  // Rematch brings the room back, with everyone still seated.
  await creator.getByRole("button", { name: "Rematch" }).click();
  await expect(creator.locator("#room-scene")).toBeVisible();
  await expect(creator.locator("li[data-slot]")).toHaveCount(TABS);
  await expect(creator.locator("li[data-slot='0']")).toHaveAttribute("data-state", "unready");
  await expect(creator.locator("li[data-slot='1']")).toHaveAttribute("data-state", "finished");

  for (const context of contexts) await context.close();
});
