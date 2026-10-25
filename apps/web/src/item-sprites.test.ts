// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { Item } from "./generated/protocol";
import {
  ITEM_SPRITES,
  itemSprite,
  MAX_SPRITE_COLOURS,
  SPRITE_OUTLINE,
  SPRITE_PALETTE,
  SPRITE_SIZE,
} from "./item-sprites";

const items = Object.keys(ITEM_SPRITES) as Item[];

describe("the item sprites", () => {
  it.each(items)("%s is a 16x16 grid painted only from the palette", (item) => {
    const grid = ITEM_SPRITES[item];
    expect(grid).toHaveLength(SPRITE_SIZE);
    for (const row of grid) {
      expect(row).toHaveLength(SPRITE_SIZE);
      for (const key of row) {
        expect(key === "." || key in SPRITE_PALETTE).toBe(true);
      }
    }
  });

  it.each(items)("%s keeps to eight colours and is outlined in the panel's brown", (item) => {
    const keys = new Set(ITEM_SPRITES[item].join("").replaceAll(".", ""));
    expect(keys.size).toBeLessThanOrEqual(MAX_SPRITE_COLOURS);
    const colours = [...keys].map((key) => SPRITE_PALETTE[key]);
    expect(colours).toContain(SPRITE_OUTLINE);
  });

  it("draws a different character for each item", () => {
    const drawings = new Set(items.map((item) => ITEM_SPRITES[item].join("\n")));
    expect(drawings.size).toBe(items.length);
  });

  it("renders one crisp rect per painted pixel, hidden from assistive technology", () => {
    const sprite = itemSprite("hazard");
    expect(sprite.getAttribute("viewBox")).toBe("0 0 16 16");
    expect(sprite.getAttribute("shape-rendering")).toBe("crispEdges");
    expect(sprite.getAttribute("aria-hidden")).toBe("true");
    // The grid is the drawing; every painted cell becomes one unit rect in its colour.
    const painted = ITEM_SPRITES.hazard.flatMap((row, y) =>
      [...row].flatMap((key, x) => (key === "." ? [] : [{ x, y, fill: SPRITE_PALETTE[key] }])),
    );
    const rects = [...sprite.querySelectorAll("rect")].map((rect) => ({
      x: Number(rect.getAttribute("x")),
      y: Number(rect.getAttribute("y")),
      fill: rect.getAttribute("fill"),
    }));
    expect(rects).toEqual(painted);
    expect(rects.length).toBeGreaterThan(0);
    expect(sprite.querySelector("rect")?.getAttribute("width")).toBe("1");
    expect(sprite.querySelector("rect")?.getAttribute("height")).toBe("1");
  });
});
