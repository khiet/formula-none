import { describe, expect, it } from "vitest";
import { finalLap, finalLapBlend } from "./final-lap";
import { OPEN_SKY } from "./presentation-zones";
import { ON_THE_GRID, raceReadout } from "./race-readout-fixture";

const LAPS = 3;

/** The own kart, slot 0, on `lap` of the race, finished or not, in `phase`. */
function onLap(
  lap: number,
  finishTimeS: number | null = null,
  phase = raceReadout().phase,
): ReturnType<typeof raceReadout> {
  return raceReadout({ standings: [{ ...ON_THE_GRID, lap, finishTimeS }], phase });
}

describe("finalLap", () => {
  it("is off before the last lap", () => {
    expect(finalLap(onLap(1), 0, LAPS)).toBe(false);
    expect(finalLap(onLap(LAPS - 1), 0, LAPS)).toBe(false);
  });

  it("is on from the last lap's first tick", () => {
    expect(finalLap(onLap(LAPS), 0, LAPS)).toBe(true);
  });

  it("is off once the own kart has finished, though its lap stays the last", () => {
    expect(finalLap(onLap(LAPS, 92.4), 0, LAPS)).toBe(false);
  });

  it("is off once the race is over for everyone", () => {
    expect(finalLap(onLap(LAPS, null, { kind: "finished" }), 0, LAPS)).toBe(false);
  });

  it("reads the own kart's lap, not the leader's", () => {
    const race = raceReadout({
      standings: [
        { ...ON_THE_GRID, lap: LAPS - 1 },
        { ...ON_THE_GRID, lap: LAPS },
      ],
    });
    expect(finalLap(race, 0, LAPS)).toBe(false);
    expect(finalLap(race, 1, LAPS)).toBe(true);
  });
});

describe("finalLapBlend", () => {
  it("leaves the zone's blend alone off the final lap", () => {
    expect(finalLapBlend(OPEN_SKY, false)).toBe(OPEN_SKY);
  });

  it("brings the crowd up everywhere on the final lap, louder than any stand's zone", () => {
    const lifted = finalLapBlend(OPEN_SKY, true);
    expect(lifted.crowd).toBe(1);
    expect(finalLapBlend({ ...OPEN_SKY, crowd: 0.6 }, true).crowd).toBe(1);
    expect(lifted.light).toBe(OPEN_SKY.light);
  });
});
