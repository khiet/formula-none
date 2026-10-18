import { expect, test } from "@playwright/test";

test("the Title Scene loads with the way into a race", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle("Formula None");
  await expect(page.getByRole("heading", { name: "Formula None" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Race" })).toBeVisible();
});
