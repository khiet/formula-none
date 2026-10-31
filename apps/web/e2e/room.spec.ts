import { type BrowserContext, expect, type Page, test } from "@playwright/test";

// Two friends in real browsers against the real server behind the dev
// server's /api proxy (ADR 0012): one creates a room, the other joins from
// the shared link in a second browser context, with its own localStorage,
// both reach the Room Scene, and the creator's Start takes both into the
// race's countdown.

const CODE = /^[A-HJ-KM-NP-Z]{4}$/;

async function roomCode(page: Page): Promise<string> {
  const code = (await page.locator("[data-room-code]").textContent()) ?? "";
  expect(code).toMatch(CODE);
  return code;
}

test("a friend joins the room from the shared link and both can race", async ({
  page,
  browser,
}) => {
  await page.goto("/");
  await page.getByLabel("Nickname").fill("khiet");
  await page.getByRole("button", { name: "Create room" }).click();
  await expect(page.locator("#room-scene")).toBeVisible({ timeout: 15_000 });
  const code = await roomCode(page);
  await expect(page).toHaveURL(new RegExp(`\\?room=${code}$`));
  await expect(page.locator("li[data-slot='0']")).toContainText("khiet");
  await expect(page.locator("li[data-slot='0']")).toContainText("(creator)");
  await expect(page.getByRole("button", { name: "Start" })).toBeDisabled();
  // The nickname is remembered for the next visit.
  expect(await page.evaluate(() => window.localStorage.getItem("nickname"))).toBe("khiet");

  const friendContext: BrowserContext = await browser.newContext();
  const friend = await friendContext.newPage();
  const link = (await page.locator("a[data-room-link]").getAttribute("href")) ?? "";
  expect(link).toContain(`?room=${code}`);
  // No nickname yet in this context: the Title asks for one with the code filled in.
  await friend.goto(link);
  await expect(friend.getByLabel("Room code")).toHaveValue(code);
  await friend.getByLabel("Nickname").fill("ada");
  await friend.getByRole("button", { name: "Join room" }).click();
  await expect(friend.locator("#room-scene")).toBeVisible({ timeout: 15_000 });
  expect(await roomCode(friend)).toBe(code);

  // Both see both, with the liveries in join order.
  for (const someone of [page, friend]) {
    await expect(someone.locator("li[data-slot]")).toHaveCount(2);
    await expect(someone.locator("li[data-slot='1']")).toContainText("ada");
    await expect(someone.locator("li[data-slot='0'] .livery")).toHaveAttribute("data-livery", "0");
    await expect(someone.locator("li[data-slot='1'] .livery")).toHaveAttribute("data-livery", "1");
  }
  await expect(friend.getByRole("button", { name: "Start" })).toBeHidden();

  // Start enables once every human is ready, and takes everyone into the race.
  await friend.getByRole("button", { name: "Ready", exact: true }).click();
  await expect(page.locator("li[data-slot='1']")).toHaveAttribute("data-state", "ready");
  await expect(page.getByRole("button", { name: "Start" })).toBeDisabled();
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

  // The server runs the race (ADR 0009): the six bots behind the two idle
  // humans drive past them, and the creator's throttle reaches the server
  // and moves their kart in everyone's snapshots.
  for (const someone of [page, friend]) {
    await expect(someone.locator('[data-instrument="lap"]')).toHaveText("Lap 1/3");
    await expect(someone.locator('[data-instrument="position"]')).toHaveText(/^[78]th$/, {
      timeout: 90_000,
    });
  }
  await page.keyboard.down("ArrowUp");
  const speed = page.locator('[data-instrument="speed"]');
  await expect
    .poll(async () => Number.parseFloat((await speed.textContent()) ?? "0"), { timeout: 20_000 })
    .toBeGreaterThan(18);
  await page.keyboard.up("ArrowUp");
  await friendContext.close();
});

test("the creator's course is the room's: both Room Scenes show it and the race runs it", async ({
  page,
  browser,
}) => {
  await page.goto("/");
  await page.getByLabel("Nickname").fill("khiet");
  // The radio sits under its flag tile, which takes the click.
  await page.locator(".flag-tile", { hasText: "Goldstone" }).click();
  await expect(page.getByRole("radio", { name: "Goldstone" })).toBeChecked();
  await page.getByRole("button", { name: "Create room" }).click();
  await expect(page.locator("#room-scene")).toBeVisible({ timeout: 15_000 });
  const code = await roomCode(page);

  // A joiner's own pick, left at the default, does not change the room's course.
  const friendContext = await browser.newContext();
  const friend = await friendContext.newPage();
  await friend.goto("/");
  await expect(friend.getByRole("radio", { name: "Bell Deer" })).toBeChecked();
  await friend.getByLabel("Nickname").fill("ada");
  await friend.getByLabel("Room code").fill(code);
  await friend.getByRole("button", { name: "Join room" }).click();
  await expect(friend.locator("#room-scene")).toBeVisible({ timeout: 15_000 });

  for (const someone of [page, friend]) {
    await expect(someone.locator("[data-room-course]")).toHaveText("Goldstone");
  }

  // The Countdown names the course: each client loads Goldstone's mesh to race it.
  const meshes = [page, friend].map((someone) =>
    someone.waitForRequest(/goldstone\.glb/, { timeout: 60_000 }),
  );
  await friend.getByRole("button", { name: "Ready", exact: true }).click();
  await page.getByRole("button", { name: "Ready", exact: true }).click();
  await page.getByRole("button", { name: "Start" }).click();
  await Promise.all(meshes);
  await friendContext.close();
});

test("a wrong code, a short nickname and a shared link all say what to do", async ({ page }) => {
  await page.goto("/?room=ZZZZ");
  await expect(page.getByLabel("Room code")).toHaveValue("ZZZZ");
  await page.getByLabel("Nickname").fill("k");
  await page.getByRole("button", { name: "Join room" }).click();
  await expect(page.locator("[data-message]")).toHaveText("Nickname needs 2 to 12 characters");
  await page.getByLabel("Nickname").fill("khiet");
  await page.getByRole("button", { name: "Join room" }).click();
  await expect(page.locator("[data-message]")).toHaveText("No room has that code", {
    timeout: 15_000,
  });
  await expect(page.getByLabel("Room code")).toHaveValue("ZZZZ");
});
