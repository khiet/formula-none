import { describe, expect, it } from "vitest";
import type { KartReadout, KartSize, MiniTurbo } from "./bridge";
import { createExhaustFlames } from "./exhaust-flame";
import { kartReadout } from "./kart-readout-fixture";

const SIZE: KartSize = { width: 1.2, height: 0.6, length: 1.8 };

function drifting(miniTurbo: MiniTurbo, rest: Partial<KartReadout> = {}): KartReadout {
  return kartReadout({ drift: { direction: "left", chargeS: 0 }, miniTurbo, ...rest });
}

function boosting(boostS: number, rest: Partial<KartReadout> = {}): KartReadout {
  return kartReadout({ boostS, ...rest });
}

/** The one kart's flame after reading `readings` in turn, each with whether the count was running. */
function flameAfter(...readings: (readonly [KartReadout, boolean] | KartReadout)[]) {
  const flames = createExhaustFlames(1, SIZE);
  for (const reading of readings) {
    const [kart, countingDown] = Array.isArray(reading) ? reading : [reading, false];
    flames.update([kart], countingDown);
  }
  const [flame] = flames.flames;
  return flame;
}

describe("the exhaust flame", () => {
  it("does not burn off a boost", () => {
    expect(flameAfter(kartReadout())?.visible).toBe(false);
    expect(flameAfter(drifting("red"))?.visible).toBe(false);
  });

  it("burns orange, at full strength, for an orange drift's release", () => {
    const flame = flameAfter(drifting("orange"), boosting(0.6));
    expect(flame?.visible).toBe(true);
    expect(flame?.tier).toBe("orange");
    expect(flame?.strength).toBe(1);
  });

  it("burns red for a red drift's release", () => {
    expect(flameAfter(drifting("red"), boosting(1))?.tier).toBe("red");
  });

  it("burns orange for a weak rocket start and red for a full one, from Go", () => {
    const weak = flameAfter(
      [kartReadout({ start: "weakBoost" }), true],
      boosting(0.6, { start: "weakBoost" }),
    );
    expect(weak?.tier).toBe("orange");
    const full = flameAfter(
      [kartReadout({ start: "fullBoost" }), true],
      boosting(1.5, { start: "fullBoost" }),
    );
    expect(full?.tier).toBe("red");
  });

  it("burns red for a boost no release explains: a pad or an item", () => {
    expect(flameAfter(kartReadout(), boosting(1))?.tier).toBe("red");
    // A weak start's verdict stands on the transform all race; a pad later is still red.
    expect(
      flameAfter(kartReadout({ start: "weakBoost" }), boosting(1, { start: "weakBoost" }))?.tier,
    ).toBe("red");
  });

  it("burns orange for an orange release that cuts a longer pad boost short", () => {
    const flames = createExhaustFlames(1, SIZE);
    flames.update([drifting("orange", { boostS: 0.8 })], false);
    flames.update([boosting(0.58)], false);
    expect(flames.flames[0]?.tier).toBe("orange");
    expect(flames.flames[0]?.strength).toBe(1);
  });

  it("burns by the rocket start's verdict at Go, not the hold's last charge", () => {
    const crossedToFull = flameAfter(
      [kartReadout({ start: "weakBoost" }), true],
      boosting(1.5, { start: "fullBoost" }),
    );
    expect(crossedToFull?.tier).toBe("red");
    const crossedToWeak = flameAfter(
      [kartReadout({ start: "clean" }), true],
      boosting(0.6, { start: "weakBoost" }),
    );
    expect(crossedToWeak?.tier).toBe("orange");
  });

  it("keeps its tier for the whole boost and takes the new source's when a boost lengthens it", () => {
    const flames = createExhaustFlames(1, SIZE);
    flames.update([drifting("orange")], false);
    flames.update([boosting(0.6)], false);
    flames.update([boosting(0.4)], false);
    expect(flames.flames[0]?.tier).toBe("orange");
    // A pad mid-boost tops it up.
    flames.update([boosting(1)], false);
    expect(flames.flames[0]?.tier).toBe("red");
  });

  it("shrinks as the boost runs out and fades away over its last quarter", () => {
    const flames = createExhaustFlames(1, SIZE);
    flames.update([drifting("red")], false);
    const strengths = [1, 0.75, 0.5, 0.25, 0.1, 0.02].map((boostS) => {
      flames.update([boosting(boostS)], false);
      return flames.flames[0]?.strength ?? Number.NaN;
    });
    expect(strengths[0]).toBe(1);
    // Half the boost left: three quarters of full size.
    expect(strengths[2]).toBeCloseTo(0.75);
    strengths.slice(1).forEach((strength, index) => {
      expect(strength).toBeLessThan(strengths[index] ?? 0);
    });
    expect(strengths[5]).toBeLessThan(0.1);
    flames.update([kartReadout()], false);
    expect(flames.flames[0]?.visible).toBe(false);
  });

  it("sits at the middle of the kart's tail and points back out of it", () => {
    const flame = flameAfter(
      drifting("red", { position: [5, 1, 5] }),
      boosting(1, { position: [5, 1, 5] }),
    );
    // A kart with no yaw faces -Z, so its tail is toward +Z.
    expect(flame?.position).toEqual([5, 1, 5.9]);
    expect(flame?.back).toEqual([0, 0, 1]);
  });

  it("puts out the flame of a slot the sim did not report", () => {
    const flames = createExhaustFlames(2, SIZE);
    flames.update([boosting(1), boosting(1)], false);
    flames.update([boosting(0.9)], false);
    expect(flames.flames.map((flame) => flame.visible)).toEqual([true, false]);
  });
});
