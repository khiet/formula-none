import { expect, test } from "@playwright/test";

test("title screen loads", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle("Formula None");
});
