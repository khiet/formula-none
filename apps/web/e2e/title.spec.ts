import { expect, test } from "@playwright/test";

test("the Title Scene loads with the ways into a room and a race", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle("Formula None");
  await expect(page.getByRole("heading", { name: "Formula None" })).toBeVisible();
  await expect(page.getByLabel("Nickname")).toBeVisible();
  await expect(page.getByRole("button", { name: "Create room" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Join room" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Solo race" })).toBeVisible();
  for (const toggle of ["Sound effects", "Engine", "Music"]) {
    await expect(page.getByLabel(toggle)).toBeChecked();
  }
});

test("an audio toggle turned off stays off after a reload", async ({ page }) => {
  await page.goto("/");
  await page.getByLabel("Music").uncheck();
  await page.reload();
  await expect(page.getByLabel("Music")).not.toBeChecked();
  await expect(page.getByLabel("Engine")).toBeChecked();
});
