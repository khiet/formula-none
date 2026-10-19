import { expect, type Page } from "@playwright/test";

/** From the Title Scene into a solo race on `url`'s track. */
export async function enterRace(page: Page, url: string): Promise<void> {
  await page.goto(url);
  await page.getByRole("button", { name: "Solo race" }).click();
}

/**
 * Into a race and through its countdown: the sim holds the karts until Go,
 * so nothing drives before the count is over. A CI runner steps the sim far
 * slower than real time, so the wait is sized in wall time with room to
 * spare.
 */
export async function startRace(page: Page, url: string): Promise<void> {
  await enterRace(page, url);
  const countdown = page.locator('[data-instrument="countdown"]');
  await expect(countdown).toHaveText(/^[321]$/);
  await expect(countdown).toBeHidden({ timeout: 60_000 });
}
