import { describe, expect, it } from "vitest";
import type { KartReadout, KartSize, MiniTurbo } from "./bridge";
import { createExhaustFlames } from "./exhaust-flame";
import { kartReadout } from "./kart-readout-fixture";

const SIZE: KartSize = { width: 1.2, height: 0.6, length: 1.8 };

function drifting(miniTurbo: MiniTurbo, rest: Partial<KartReadout> = {}): KartReadout {
  return kartReadout({ drift: { direction: "left", chargeS: 0 }, miniTurbo, ...rest });
}

function boosting(
  boostS: number,
  boostTier: MiniTurbo = "red",
  rest: Partial<KartReadout> = {},
): KartReadout {
  return kartReadout({ boostS, boostTier, ...rest });
}

/** The one kart's flame after reading `readings` in turn. */
function flameAfter(...readings: KartReadout[]) {
  const flames = createExhaustFlames(1, SIZE);
  for (const kart of readings) {
    flames.update([kart]);
  }
  const [flame] = flames.flames;
  return flame;
}

describe("the exhaust flame", () => {
  it("does not burn off a boost", () => {
    expect(flameAfter(kartReadout())?.visible).toBe(false);
    expect(flameAfter(drifting("red"))?.visible).toBe(false);
  });

  it("burns in the tier the boost reports, at full strength as it begins", () => {
    const orange = flameAfter(boosting(0.6, "orange"));
    expect(orange?.visible).toBe(true);
    expect(orange?.tier).toBe("orange");
    expect(orange?.strength).toBe(1);
    expect(flameAfter(boosting(1, "red"))?.tier).toBe("red");
  });

  it("starts over at full strength for a release that cuts a longer boost short", () => {
    const flames = createExhaustFlames(1, SIZE);
    // A red release in an item's longer red boost.
    flames.update([drifting("red", { boostS: 1.3, boostTier: "red" })]);
    flames.update([boosting(0.98, "red")]);
    expect(flames.flames[0]?.strength).toBe(1);
  });

  it("takes the new boost's tier and starts over when another boost lengthens it", () => {
    const flames = createExhaustFlames(1, SIZE);
    flames.update([boosting(0.6, "orange")]);
    flames.update([boosting(0.4, "orange")]);
    expect(flames.flames[0]?.tier).toBe("orange");
    // A pad mid-boost tops it up.
    flames.update([boosting(1, "red")]);
    expect(flames.flames[0]?.tier).toBe("red");
    expect(flames.flames[0]?.strength).toBe(1);
  });

  it("shrinks as the boost runs out and fades away over its last quarter", () => {
    const flames = createExhaustFlames(1, SIZE);
    const strengths = [1, 0.75, 0.5, 0.25, 0.1, 0.02].map((boostS) => {
      flames.update([boosting(boostS)]);
      return flames.flames[0]?.strength ?? Number.NaN;
    });
    expect(strengths[0]).toBe(1);
    // Half the boost left: three quarters of full size.
    expect(strengths[2]).toBeCloseTo(0.75);
    strengths.slice(1).forEach((strength, index) => {
      expect(strength).toBeLessThan(strengths[index] ?? 0);
    });
    expect(strengths[5]).toBeLessThan(0.1);
    flames.update([kartReadout()]);
    expect(flames.flames[0]?.visible).toBe(false);
  });

  it("sits at the middle of the kart's tail and points back out of it", () => {
    const flame = flameAfter(boosting(1, "red", { position: [5, 1, 5] }));
    // A kart with no yaw faces -Z, so its tail is toward +Z.
    expect(flame?.position).toEqual([5, 1, 5.9]);
    expect(flame?.back).toEqual([0, 0, 1]);
  });

  it("puts out the flame of a slot the sim did not report", () => {
    const flames = createExhaustFlames(2, SIZE);
    flames.update([boosting(1), boosting(1)]);
    flames.update([boosting(0.9)]);
    expect(flames.flames.map((flame) => flame.visible)).toEqual([true, false]);
  });
});
