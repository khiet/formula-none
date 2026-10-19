import { expect, test } from "@playwright/test";

test("the Title Scene loads with the ways into a room and a race", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle("Formula None");
  await expect(page.getByRole("heading", { name: "Formula None" })).toBeVisible();
  await expect(page.getByLabel("Nickname")).toBeVisible();
  await expect(page.getByRole("button", { name: "Create room" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Join room" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Solo race" })).toBeVisible();
});
