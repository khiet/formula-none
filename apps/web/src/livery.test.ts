import { describe, expect, it } from "vitest";
import { LIVERIES, liveryColour, liveryCss } from "./livery";

describe("liveries", () => {
  it("give each of the eight slots its own hue around the wheel", () => {
    const hues = Array.from({ length: LIVERIES }, (_, slot) => liveryColour(slot).hue);
    expect(new Set(hues).size).toBe(LIVERIES);
    expect(hues[0]).toBe(0);
    expect(Math.max(...hues)).toBeLessThan(1);
  });

  it("print as CSS for the Room Scene", () => {
    expect(liveryCss(0)).toBe("hsl(0deg 80% 50%)");
    expect(liveryCss(2)).toBe("hsl(90deg 80% 50%)");
  });
});
