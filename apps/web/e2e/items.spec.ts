import { expect, test } from "@playwright/test";
import { enterRace } from "./scenes";

// An item box under the own kart's grid slot, taken at Go: the item rolls
// in the slot before it lands. The oval's sidecar is rewritten in the page
// as it is fetched; page.route would also catch the module Vite serves the
// sidecar's URL from.
test("an item box taken at Go rolls, then lands in the slot", async ({ page }) => {
  await page.addInitScript(() => {
    const fetchOriginal = window.fetch.bind(window);
    window.fetch = async (input, init) => {
      const response = await fetchOriginal(input, init);
      const url = input instanceof Request ? input.url : String(input);
      if (!url.includes("oval.track.json")) return response;
      const sidecar = await response.json();
      const [slot] = sidecar.spawn_grid;
      sidecar.item_boxes.push({ position: slot.position, yaw: slot.yaw });
      return new Response(JSON.stringify(sidecar), { headers: response.headers });
    };
  });
  await enterRace(page, "/?track=oval");
  const item = page.locator('[data-instrument="item"]');
  await expect(item).toHaveAttribute("data-item-state", "empty");
  await expect(page.locator('[data-instrument="countdown"]')).toContainText("GO", {
    timeout: 60_000,
  });
  // Taken on Go's tick, the item rolls for 2.5 s: the slot shows nothing
  // yet, then the item. A CI runner steps the sim slower than real time,
  // so the wait for the landing is sized in wall time with room to spare.
  await expect(item).toHaveAttribute("data-item-state", "empty");
  await expect(item).toHaveAttribute("data-item-state", "held", { timeout: 10_000 });
  await expect(item).toHaveAttribute("data-item", /^(boost|hazard|homing)$/);
});
