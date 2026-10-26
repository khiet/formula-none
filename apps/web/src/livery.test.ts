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
  const all = Array.from({ length: LIVERIES }, (_, slot) => livery(slot));

  it("are eight, each a key colour of its own", () => {
    expect(all).toHaveLength(8);
    expect(new Set(all.map(({ key }) => key)).size).toBe(8);
  });

  it("tell every pair apart at race distance by their key colours", () => {
    for (let a = 0; a < LIVERIES; a += 1) {
      for (let b = a + 1; b < LIVERIES; b += 1) {
        const distance = deltaE(livery(a).key, livery(b).key);
        expect(distance, `livery ${a} vs ${b}`).toBeGreaterThanOrEqual(RACE_DISTANCE_DELTA_E);
      }
    }
  });

  it("paint the car in the team's colours, the helmet in its accent", () => {
    expect(livery(0)).toMatchObject({
      name: "Rosso",
      primary: 0xd00a12,
      secondary: 0x141414,
      accent: 0xffd400,
      helmet: 0xffd400,
    });
    for (const paint of all) {
      expect(paint.helmet, paint.name).toBe(paint.accent);
    }
  });

  it("seat each slot's driver under a parody name", () => {
    expect(all.map(({ driver }) => driver.name)).toEqual([
      "Louis Hamiltoon",
      "Max Verstoppen",
      "Georgie Rustle",
      "Lando Nourish",
      "Fernando Alonsno",
      "Carlos Saintz",
      "Pierre Gassly",
      "Nico Hulkenbird",
    ]);
  });

  it("print the key colour as CSS for the Room Scene's swatches", () => {
    expect(liveryCss(0)).toBe("#d00a12");
    expect(liveryCss(7)).toBe("#2a2a2e");
  });
});
