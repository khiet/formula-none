import { expect, test } from "@playwright/test";

test("the Title Scene loads with the ways into a room and a race", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle("Formula None");
  await expect(page.getByRole("heading", { name: "Formula None" })).toBeVisible();
  await expect(page.getByLabel("Nickname")).toBeVisible();
  await expect(page.getByRole("button", { name: "Create room" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Join room" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Solo race" })).toBeVisible();
  for (const course of ["Bell Deer", "Goldstone", "Monarch"]) {
    await expect(page.locator(".flag-tile", { hasText: course }).locator("svg")).toBeVisible();
  }
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

test("the picked course is remembered and a solo race runs it", async ({ page }) => {
  await page.goto("/");
  await page.locator(".flag-tile", { hasText: "Goldstone" }).click();
  await page.reload();
  await expect(page.getByRole("radio", { name: "Goldstone" })).toBeChecked();
  const mesh = page.waitForRequest(/goldstone\.glb/, { timeout: 30_000 });
  await page.getByRole("button", { name: "Solo race" }).click();
  await mesh;
});
