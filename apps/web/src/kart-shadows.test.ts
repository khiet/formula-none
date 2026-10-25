import { describe, expect, it } from "vitest";
import type { KartReadout } from "./bridge";
import { kartReadout, yawed } from "./kart-readout-fixture";
import { createKartShadows, type SurfaceProbe } from "./kart-shadows";

/** A kart at `position` turned `yaw` radians about +Y from its -Z facing. */
function kart(yaw: number, position: readonly [number, number, number]): KartReadout {
  return kartReadout({ position, rotation: yawed(yaw), airborne: true });
}

/** Flat ground at `groundY` everywhere, or nothing at all when it is undefined. */
function flatGround(groundY: number | undefined): SurfaceProbe {
  return {
    under: (x, _y, z) =>
      groundY === undefined ? undefined : { point: [x, groundY, z], normal: [0, 1, 0] },
  };
}

function expectClose(actual: readonly number[], expected: readonly number[], digits = 6) {
  expect(actual).toHaveLength(expected.length);
  actual.forEach((value, index) => {
    expect(value, `component ${index}`).toBeCloseTo(expected[index] ?? Number.NaN, digits);
  });
}

describe("the kart shadows", () => {
  it("sits on the surface under a hopping kart, not at the kart's height", () => {
    const shadows = createKartShadows(1, flatGround(0));
    shadows.update([kart(0, [3, 0.41, -7])]);
    const disc = shadows.discs[0];
    expect(disc?.visible).toBe(true);
    expectClose(disc?.position ?? [], [3, 0, -7]);
    expectClose(disc?.normal ?? [], [0, 1, 0]);
  });

  it("points the disc along the kart's heading", () => {
    const shadows = createKartShadows(1, flatGround(0));
    shadows.update([kart(-Math.PI / 2, [0, 0.3, 0])]);
    // A quarter turn to the right from -Z faces +X.
    expectClose(shadows.discs[0]?.forward ?? [], [1, 0, 0]);
  });

  it("shadows every kart on the grid, remote ones included", () => {
    const shadows = createKartShadows(3, flatGround(1.5));
    shadows.update([kart(0, [0, 1.8, 0]), kart(0, [4, 1.9, 2]), kart(0, [8, 1.8, 4])]);
    expect(shadows.discs.map((disc) => disc.visible)).toEqual([true, true, true]);
    expectClose(shadows.discs[2]?.position ?? [], [8, 1.5, 4]);
  });

  it("hides the disc of a kart with nothing below it", () => {
    const shadows = createKartShadows(2, flatGround(undefined));
    shadows.update([kart(0, [0, 0.3, 0]), kart(0, [4, 0.3, 0])]);
    expect(shadows.discs.map((disc) => disc.visible)).toEqual([false, false]);
  });

  it("hides the discs of slots the sim did not report", () => {
    const shadows = createKartShadows(2, flatGround(0));
    shadows.update([kart(0, [0, 0.3, 0])]);
    expect(shadows.discs.map((disc) => disc.visible)).toEqual([true, false]);
  });
});
