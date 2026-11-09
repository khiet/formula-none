import { describe, expect, it } from "vitest";
import { OPEN_SKY } from "./presentation-zones";
import { ON_THE_GRID } from "./race-readout-fixture";
import { cloudCover, rainBlend, rainLook, rainPlateShowing, rainSkyLight } from "./rain-look";

const LAP_M = 1000;
const at = (lap: number, splineProgress: number, checkpoint = 3) => ({
  ...ON_THE_GRID,
  lap,
  splineProgress,
  checkpoint,
});

describe("cloudCover", () => {
  it("is clear with no rain lap, wherever the karts are", () => {
    expect(cloudCover([at(3, 900)], null, LAP_M)).toBe(0);
  });

  it("gathers over the last quarter of the lap before the rain's, from none to full", () => {
    expect(cloudCover([at(2, 749)], 3, LAP_M)).toBe(0);
    expect(cloudCover([at(2, 750)], 3, LAP_M)).toBe(0);
    expect(cloudCover([at(2, 875)], 3, LAP_M)).toBeCloseTo(0.5);
    expect(cloudCover([at(2, 1000)], 3, LAP_M)).toBeCloseTo(1);
  });

  it("is clear before the lap before and full from the rain's lap on", () => {
    expect(cloudCover([at(1, 950)], 3, LAP_M)).toBe(0);
    expect(cloudCover([at(3, 10)], 3, LAP_M)).toBe(1);
    expect(cloudCover([at(4, 10)], 3, LAP_M)).toBe(1);
  });

  it("follows the leader, whoever that is, however the rest of the field runs", () => {
    expect(cloudCover([at(2, 100), at(2, 875), at(1, 900)], 3, LAP_M)).toBeCloseTo(0.5);
    expect(cloudCover([at(1, 900), at(3, 1)], 3, LAP_M)).toBe(1);
  });

  it("takes no kart behind the line on the grid for one at the end of its lap", () => {
    // Lap 1 of a race raining on lap 2, a few metres short of the line at Go.
    expect(cloudCover([at(1, 990, 0)], 2, LAP_M)).toBe(0);
    expect(cloudCover([at(1, 990, 1)], 2, LAP_M)).toBeCloseTo(0.96);
  });
});

describe("rainLook", () => {
  it("leaves a dry race's look as it is", () => {
    expect(rainLook(0, 0, 0)).toEqual({ clouds: 0, wetness: 0, overcast: 0, streaks: 0 });
  });

  it("greys part way under the gathering clouds, before any wet", () => {
    const look = rainLook(0, 1, 0);
    expect(look.overcast).toBeGreaterThan(0);
    expect(look.overcast).toBeLessThan(1);
    expect(look.wetness).toBe(0);
    expect(look.streaks).toBe(0);
  });

  it("is wholly grey and streaking once the track is wholly wet", () => {
    expect(rainLook(1, 1, 0)).toEqual({ clouds: 1, wetness: 1, overcast: 1, streaks: 1 });
  });

  it("keeps the clouds up in the wet, as a rejoin mid-rain reads none gathered", () => {
    expect(rainLook(0.1, 0, 0).clouds).toBe(1);
  });

  it("stops the streaks under a roof and keeps the wet", () => {
    const under = rainLook(1, 1, 1);
    expect(under.streaks).toBe(0);
    expect(under.wetness).toBe(1);
    expect(under.overcast).toBe(1);
    expect(rainLook(1, 1, 0.5).streaks).toBeCloseTo(0.5);
  });
});

describe("rainBlend", () => {
  it("leaves the zone's blend alone in the dry", () => {
    expect(rainBlend(OPEN_SKY, rainLook(0, 0, 0))).toBe(OPEN_SKY);
  });

  it("scales the light under the overcast, on top of the zone's, and nothing else", () => {
    const tunnel = { ...OPEN_SKY, light: 0.5, echo: 1 };
    const wet = rainBlend(tunnel, rainLook(1, 1, 0));
    expect(wet.light).toBeCloseTo(0.5 * rainSkyLight(1));
    expect(rainSkyLight(1)).toBeLessThan(1);
    expect(rainSkyLight(1)).toBeGreaterThan(0);
    expect(wet.echo).toBe(1);
  });
});

describe("rainPlateShowing", () => {
  it("shows as the rain arrives and not before or for long", () => {
    expect(rainPlateShowing(0)).toBe(false);
    expect(rainPlateShowing(0.01)).toBe(true);
    expect(rainPlateShowing(0.2)).toBe(true);
    expect(rainPlateShowing(0.5)).toBe(false);
    expect(rainPlateShowing(1)).toBe(false);
  });
});
