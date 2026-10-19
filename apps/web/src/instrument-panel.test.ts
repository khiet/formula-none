// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { Standing } from "./bridge";
import {
  CONNECTION_LOST_BANNER,
  createInstrumentPanel,
  lapReadouts,
  phaseReadout,
} from "./instrument-panel";

const onTheGrid: Standing = {
  position: 1,
  lap: 1,
  checkpoint: 0,
  splineProgress: 434,
  lapTimeSeconds: 0,
  bestLapSeconds: null,
  wrongWay: false,
  finishTimeSeconds: null,
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

describe("phase readout", () => {
  it("counts 3, 2, 1 down the countdown, whole seconds rounded up", () => {
    expect(phaseReadout({ kind: "countdown", remainingSeconds: 3 })).toBe("3");
    expect(phaseReadout({ kind: "countdown", remainingSeconds: 2.01 })).toBe("3");
    expect(phaseReadout({ kind: "countdown", remainingSeconds: 2 })).toBe("2");
    expect(phaseReadout({ kind: "countdown", remainingSeconds: 0.3 })).toBe("1");
  });

  it("shows GO for the first second of the race and nothing after", () => {
    expect(phaseReadout({ kind: "racing", elapsedSeconds: 0, graceLeftSeconds: null })).toBe("GO");
    expect(phaseReadout({ kind: "racing", elapsedSeconds: 0.9, graceLeftSeconds: null })).toBe(
      "GO",
    );
    expect(phaseReadout({ kind: "racing", elapsedSeconds: 1, graceLeftSeconds: null })).toBe("");
    expect(phaseReadout({ kind: "racing", elapsedSeconds: 40, graceLeftSeconds: 12 })).toBe("");
    expect(phaseReadout({ kind: "finished" })).toBe("");
  });
});

describe("the connection banner", () => {
  it("is down until the connection is lost, and down again once it is back", () => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(container, 3);
    const banner = container.querySelector<HTMLElement>('[data-instrument="connection-banner"]');
    expect(banner?.textContent).toBe(CONNECTION_LOST_BANNER);
    expect(banner?.hidden).toBe(true);
    panel.connectionLost(true);
    expect(banner?.hidden).toBe(false);
    panel.connectionLost(false);
    expect(banner?.hidden).toBe(true);
  });
});
