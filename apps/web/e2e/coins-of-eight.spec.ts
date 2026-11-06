import { readFileSync } from "node:fs";
import { type BrowserContext, expect, type Page, test } from "@playwright/test";
import { loadBridge } from "../src/bridge";
import type { ServerMessage } from "../src/generated/protocol";
import { autopilot, centreline } from "./autopilot";

// Eight tabs in one room, each driven by its own autopilot and firing
// whatever item it holds, so karts take coins and spin each other out.
// Every tab's own WebSocket must carry the same coin count for every kart
// at every tick, each loss must be the tuned three or all the kart carried,
// and each tab's coin count must read its own kart's. Eight WebGL tabs is a
// local smoke test, not a CI job.

test.skip(!!process.env.CI, "eight WebGL tabs racing for coins are a local smoke test");
// Headed, so the tabs render on the GPU and the autopilots keep to the line.
test.use({ headless: false });
test.setTimeout(900_000);

const WASM = new URL("../src/generated/bridge/bridge_bg.wasm", import.meta.url);
const RUN_MS = 150_000;
/** The coins a spin-out costs, as the sim tunes it (ADR 0055). */
const LOST_PER_SPIN_OUT = 3;

interface Seen {
  kart: number;
  /** Tick to every kart's count, from this tab's own socket. */
  counts: Map<number, number[]>;
  /** Each loss with what the kart carried just before it. */
  losses: { kart: number; count: number; carried: number }[];
  picks: number;
}

function watch(page: Page, bridge: Awaited<ReturnType<typeof loadBridge>>): Seen {
  const seen: Seen = { kart: -1, counts: new Map(), losses: [], picks: 0 };
  let slot = -1;
  // The newest snapshot's counts, moved on by the events since it.
  let carried: number[] = [];
  page.on("websocket", (socket) => {
    socket.on("framereceived", ({ payload }) => {
      if (typeof payload === "string") return;
      let message: ServerMessage;
      try {
        message = bridge.decodeServerMessage(new Uint8Array(payload));
      } catch {
        return;
      }
      if (typeof message === "string") return;
      if ("joined" in message) slot = message.joined.slot;
      else if ("countdown" in message) seen.kart = message.countdown.grid.indexOf(slot);
      else if ("events" in message) {
        for (const { kind } of message.events) {
          if (typeof kind === "string") continue;
          if ("coinPicked" in kind) {
            seen.picks += 1;
            const { kart } = kind.coinPicked;
            carried[kart] = (carried[kart] ?? 0) + 1;
          }
          if ("coinsLost" in kind) {
            const { kart, count } = kind.coinsLost;
            seen.losses.push({ kart, count, carried: carried[kart] ?? 0 });
            carried[kart] = (carried[kart] ?? 0) - count;
          }
        }
      } else if ("snapshot" in message) {
        const counts = message.snapshot.karts.map((kart) => kart.coins);
        seen.counts.set(message.snapshot.tick, counts);
        carried = [...counts];
      }
    });
  });
  return seen;
}

test("eight tabs agree on every kart's coins and on every loss", async ({ browser }) => {
  const bridge = await loadBridge(readFileSync(WASM));
  const line = centreline();
  const TABS = bridge.tuning.kartsPerRace;
  const contexts: BrowserContext[] = [];
  const pages: Page[] = [];
  for (let tab = 0; tab < TABS; tab++) {
    const context = await browser.newContext({ viewport: { width: 640, height: 400 } });
    contexts.push(context);
    pages.push(await context.newPage());
  }
  const pilots = pages.map((page) => autopilot(page, bridge, line));
  const seen = pages.map((page) => watch(page, bridge));
  const creator = pages[0] as Page;

  await creator.goto("/");
  await creator.getByLabel("Nickname").fill("tab0");
  await creator.getByRole("button", { name: "Create room" }).click();
  await expect(creator.locator("#room-scene")).toBeVisible({ timeout: 15_000 });
  const code = (await creator.locator("[data-room-code]").textContent()) ?? "";
  for (const [index, page] of pages.slice(1).entries()) {
    await page.goto(`/?room=${code}`);
    await page.getByLabel("Nickname").fill(`tab${index + 1}`);
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
    await expect(page.locator('[data-instrument="coins"]')).toHaveText(
      `x00 / ${bridge.tuning.coinCap}`,
    );
  }

  // Sample each tab's coin count beside the count its own newest snapshot gives.
  const samples: { t: number; tab: number; shown: string; own: number | undefined }[] = [];
  const started = Date.now();
  while (Date.now() - started < RUN_MS) {
    if (await creator.locator("#results-scene").isVisible()) break;
    for (const [tab, page] of pages.entries()) {
      const shown = (await page.locator('[data-instrument="coins"]').textContent()) ?? "";
      samples.push({ t: Date.now() - started, tab, shown, own: pilots[tab]?.last()?.kart.coins });
      // Fire any held item at whoever is near.
      await page.keyboard.press("Shift").catch(() => {});
    }
    await creator.waitForTimeout(400);
  }

  // Every tab's socket against tab 0's, at the ticks both saw.
  const reference = seen[0] as Seen;
  let compared = 0;
  const disagreements: string[] = [];
  for (const [tab, other] of seen.slice(1).entries()) {
    for (const [tick, counts] of other.counts) {
      const theirs = reference.counts.get(tick);
      if (!theirs) continue;
      compared += 1;
      if (JSON.stringify(theirs) !== JSON.stringify(counts)) {
        disagreements.push(`tab ${tab + 1} at tick ${tick}: ${counts} vs ${theirs}`);
      }
    }
  }
  // A count that disagrees with its tab's own kart on two samples in a row;
  // one alone may straddle a snapshot.
  const stuck: string[] = [];
  const reads = (own: number) => `x${String(own).padStart(2, "0")} / ${bridge.tuning.coinCap}`;
  for (let tab = 0; tab < TABS; tab++) {
    const mine = samples.filter((sample) => sample.tab === tab && sample.own !== undefined);
    for (let index = 1; index < mine.length; index++) {
      const [a, b] = [mine[index - 1], mine[index]];
      const off = (s: typeof a) => s !== undefined && s.shown !== reads(s.own ?? 0);
      if (off(a) && off(b)) stuck.push(`tab ${tab} at ${b?.t} ms: ${b?.shown} vs ${b?.own}`);
    }
  }
  const wrongLosses = reference.losses.filter(
    (loss) => loss.count !== Math.min(LOST_PER_SPIN_OUT, loss.carried),
  );
  for (const context of contexts) await context.close();

  expect(reference.picks).toBeGreaterThan(0);
  expect(compared).toBeGreaterThan(0);
  expect(disagreements).toEqual([]);
  expect(reference.losses.length).toBeGreaterThan(0);
  expect(wrongLosses).toEqual([]);
  expect(stuck).toEqual([]);
});
