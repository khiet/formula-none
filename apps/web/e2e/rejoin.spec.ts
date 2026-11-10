import { type BrowserContext, expect, type Page, test } from "@playwright/test";

// A tab closed mid-race (ADR 0012): the server's bot drives the kart while
// it is gone, and reopening the room's link in the same browser takes the
// seat back with the session token kept for it. A new player is refused
// mid-race, so landing in the race at all is the same seat; the throttle
// moving the kart is the control handed back. The friend races on
// throughout.

async function roomCode(page: Page): Promise<string> {
  const code = (await page.locator("[data-room-code]").textContent()) ?? "";
  expect(code).toMatch(/^[A-HJ-KM-NP-Z]{4}$/);
  return code;
}

test("closing the tab mid-race and reopening the room's link rejoins the same kart", async ({
  page,
  context,
  browser,
}) => {
  await page.goto("/");
  await page.getByLabel("Nickname").fill("khiet");
  await page.getByRole("button", { name: "Create room" }).click();
  await expect(page.locator("#room-scene")).toBeVisible({ timeout: 15_000 });
  const code = await roomCode(page);

  const friendContext: BrowserContext = await browser.newContext();
  const friend = await friendContext.newPage();
  await friend.goto(`/?room=${code}`);
  await friend.getByLabel("Nickname").fill("ada");
  await friend.getByRole("button", { name: "Join room" }).click();
  await expect(friend.locator("#room-scene")).toBeVisible({ timeout: 15_000 });
  await friend.getByRole("button", { name: "Ready", exact: true }).click();
  await page.getByRole("button", { name: "Ready", exact: true }).click();
  await expect(page.getByRole("button", { name: "Start" })).toBeEnabled();
  await page.getByRole("button", { name: "Start" }).click();
  for (const someone of [page, friend]) {
    await expect(someone.locator('[data-instrument="countdown"]')).toHaveAttribute(
      "data-lit",
      /^[1-5]$/,
      {
        timeout: 60_000,
      },
    );
  }

  await page.close();
  const reopened = await context.newPage();
  await reopened.goto(`/?room=${code}`);
  // Straight into the race under way, with no Title refusing a mid-race join.
  // The track loads beside the friend's race, two browsers sharing a
  // software-rendering runner, so the wait matches the countdown's.
  await expect(reopened.locator('[data-instrument="lap"]')).toHaveText("Lap 1/3", {
    timeout: 60_000,
  });
  await expect(reopened.locator("#title-scene")).toHaveCount(0);
  await expect(reopened.locator('[data-instrument="connection-banner"]')).toBeHidden();

  await reopened.keyboard.down("ArrowUp");
  const speed = reopened.locator('[data-instrument="speed"]');
  await expect
    .poll(async () => Number.parseFloat((await speed.textContent()) ?? "0"), { timeout: 20_000 })
    .toBeGreaterThan(18);
  await reopened.keyboard.up("ArrowUp");

  // The friend never left the race.
  await expect(friend.locator('[data-instrument="lap"]')).toHaveText(/^Lap [123]\/3$/);
  await friendContext.close();
});

test("a silent server raises the connection banner and the race reconnects by itself", async ({
  page,
}) => {
  // Every WebSocket the page opens goes through here to the real server;
  // while `muted`, nothing the server sends reaches the page.
  let muted = false;
  let sockets = 0;
  await page.routeWebSocket("**/api/ws", (ws) => {
    sockets += 1;
    const server = ws.connectToServer();
    server.onMessage((message) => {
      if (!muted) ws.send(message);
    });
  });

  await page.goto("/");
  await page.getByLabel("Nickname").fill("khiet");
  await page.getByRole("button", { name: "Create room" }).click();
  await expect(page.locator("#room-scene")).toBeVisible({ timeout: 15_000 });
  await page.getByRole("button", { name: "Ready", exact: true }).click();
  await page.getByRole("button", { name: "Start" }).click();
  await expect(page.locator('[data-instrument="lap"]')).toHaveText("Lap 1/3", {
    timeout: 60_000,
  });
  const banner = page.locator('[data-instrument="connection-banner"]');
  await expect(banner).toBeHidden();
  const before = sockets;

  muted = true;
  await expect(banner).toBeVisible({ timeout: 10_000 });
  muted = false;
  await expect(banner).toBeHidden({ timeout: 10_000 });
  expect(sockets).toBeGreaterThan(before);
  // Still racing, on the kart it had.
  await expect(page.locator('[data-instrument="lap"]')).toHaveText(/^Lap [123]\/3$/);
  await expect(page.locator("#title-scene")).toHaveCount(0);
});
