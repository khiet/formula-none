// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { Item } from "./generated/protocol";
import { itemSprite } from "./item-sprites";
import { hexCss } from "./livery";
import { STAR_RAINBOW, STAR_TILES } from "./star-shine";

const items: readonly Item[] = ["boost", "hazard", "homing", "star"];

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

  it("tiles the mirror ball from the six rainbow properties, with defaults so it reads before any loop", () => {
    const ball = itemSprite("star");
    const fills = new Set(
      [...ball.querySelectorAll("polygon")]
        .map((tile) => tile.getAttribute("fill") ?? "")
        .filter((fill) => fill.startsWith("var(--")),
    );
    expect([...fills].sort()).toEqual(
      Array.from({ length: STAR_TILES }, (_, tile) => `var(--r${tile})`),
    );
    STAR_RAINBOW.forEach((stop, tile) => {
      expect(ball.style.getPropertyValue(`--r${tile}`)).toBe(hexCss(stop));
    });
  });
});
