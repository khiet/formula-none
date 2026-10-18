import { describe, expect, it } from "vitest";
import type { Standing } from "./bridge";
import { lapReadouts } from "./instrument-panel";

const onTheGrid: Standing = {
  position: 1,
  lap: 1,
  checkpoint: 0,
  splineProgress: 434,
  lapTimeSeconds: 0,
  bestLapSeconds: null,
  wrongWay: false,
};

describe("lap readouts", () => {
  it("shows the grid as first on lap one of three with the clocks blank", () => {
    expect(lapReadouts(onTheGrid, 3)).toEqual({
      position: "1st",
      lap: "Lap 1/3",
      lapTime: "0:00.000",
      bestLap: "Best --:--.---",
      wrongWay: "",
    });
  });

  it("prints the wrong-way warning only while the sim reports it", () => {
    expect(lapReadouts({ ...onTheGrid, wrongWay: true }, 3).wrongWay).toBe("WRONG WAY");
    expect(lapReadouts(onTheGrid, 3).wrongWay).toBe("");
  });

  it("shows position as an ordinal, the lap clock in minutes and the best lap once there is one", () => {
    const readouts = lapReadouts(
      { ...onTheGrid, position: 3, lap: 2, lapTimeSeconds: 75.4567, bestLapSeconds: 14.9996 },
      3,
    );
    expect(readouts.position).toBe("3rd");
    expect(readouts.lap).toBe("Lap 2/3");
    expect(readouts.lapTime).toBe("1:15.457");
    expect(readouts.bestLap).toBe("Best 0:15.000");
  });

  it("spells 2nd, 4th through 8th", () => {
    const ordinal = (position: number): string =>
      lapReadouts({ ...onTheGrid, position }, 3).position;
    expect([2, 4, 5, 8].map(ordinal)).toEqual(["2nd", "4th", "5th", "8th"]);
  });
});
