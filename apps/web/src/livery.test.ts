import { describe, expect, it } from "vitest";
import { LIVERIES, livery, liveryCss } from "./livery";

/** sRGB hex to CIE L*a*b*, the perceptual space where a distance is a difference seen. */
function lab(hex: number): [number, number, number] {
  const channel = (byte: number) => {
    const c = byte / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  const r = channel(hex >> 16);
  const g = channel((hex >> 8) & 0xff);
  const b = channel(hex & 0xff);
  const f = (t: number) => (t > 0.008856 ? Math.cbrt(t) : 7.787 * t + 16 / 116);
  const x = f((0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047);
  const y = f(0.2126 * r + 0.7152 * g + 0.0722 * b);
  const z = f((0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883);
  return [116 * y - 16, 500 * (x - y), 200 * (y - z)];
}

function deltaE(a: number, b: number): number {
  const [l1, a1, b1] = lab(a);
  const [l2, a2, b2] = lab(b);
  return Math.hypot(l1 - l2, a1 - a2, b1 - b2);
}

/**
 * A CIE76 difference of 30 is well past "clearly different colours" on a
 * calibrated display; at race distance, under the sun and on a laptop
 * panel, it is the margin that keeps two karts apart.
 */
const RACE_DISTANCE_DELTA_E = 30;

describe("liveries", () => {
  it("are eight, each a colour of its own", () => {
    const all = Array.from({ length: LIVERIES }, (_, slot) => livery(slot));
    expect(all).toHaveLength(8);
    expect(new Set(all.map(({ hex }) => hex)).size).toBe(8);
  });

  it("tell every pair apart at race distance", () => {
    for (let a = 0; a < LIVERIES; a += 1) {
      for (let b = a + 1; b < LIVERIES; b += 1) {
        const distance = deltaE(livery(a).hex, livery(b).hex);
        expect(distance, `livery ${a} vs ${b}`).toBeGreaterThanOrEqual(RACE_DISTANCE_DELTA_E);
      }
    }
  });

  it("print as CSS for the Room Scene's swatches", () => {
    expect(liveryCss(0)).toBe("#d62828");
    expect(liveryCss(7)).toBe("#202020");
  });
});
