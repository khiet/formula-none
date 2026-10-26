// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { Item } from "./generated/protocol";
import { itemSprite } from "./item-sprites";

const items: readonly Item[] = ["boost", "hazard", "homing"];

describe("the item characters", () => {
  it.each(items)(
    "draws %s in polygons, named by data-sprite and hidden from assistive technology",
    (item) => {
      const sprite = itemSprite(item);
      expect(sprite.dataset.sprite).toBe(item);
      expect(sprite.getAttribute("aria-hidden")).toBe("true");
      expect(sprite.querySelectorAll("polygon").length).toBeGreaterThan(0);
    },
  );

  it("draws a different character for each item", () => {
    const drawings = new Set(items.map((item) => itemSprite(item).innerHTML));
    expect(drawings.size).toBe(items.length);
  });
});
