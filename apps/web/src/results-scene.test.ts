// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { Standing } from "./bridge";
import { createResultsScene, resultsRows } from "./results-scene";

function standing(position: number, overrides: Partial<Standing> = {}): Standing {
  return {
    position,
    lap: 3,
    checkpoint: 0,
    splineProgress: 5,
    lapTimeSeconds: 14,
    bestLapSeconds: 14 + position,
    wrongWay: false,
    finishTimeSeconds: 45 + position,
    ...overrides,
  };
}

/** Slot 0 is the local kart; slots 1 and 2 are bots; slot 1 won and slot 0 never finished. */
const standings: readonly Standing[] = [
  standing(3, { lap: 2, bestLapSeconds: 16.2, finishTimeSeconds: null }),
  standing(1),
  standing(2),
];
const isBot = (slot: number): boolean => slot > 0;

describe("results rows", () => {
  it("lists every kart in finishing order with its name and best lap, unfinished karts included", () => {
    expect(resultsRows(standings, 0, isBot)).toEqual([
      { position: "1st", name: "Bot 2", bestLap: "0:15.000", finished: true },
      { position: "2nd", name: "Bot 3", bestLap: "0:16.000", finished: true },
      { position: "3rd", name: "You", bestLap: "0:16.200", finished: false },
    ]);
  });

  it("leaves the best lap blank for a kart that completed none", () => {
    const rows = resultsRows([standing(1, { bestLapSeconds: null })], 0, isBot);
    expect(rows[0]?.bestLap).toBe("--:--.---");
  });
});

describe("Results Scene", () => {
  it("shows the rows in order and offers Rematch and Leave", () => {
    const container = document.createElement("div");
    const scene = createResultsScene(container, resultsRows(standings, 0, isBot), {
      rematch() {},
      leave() {},
    });

    const rows = Array.from(container.querySelectorAll("[data-result]")).map(
      (row) => row.textContent,
    );
    expect(rows).toEqual([
      "1st Bot 2 0:15.000",
      "2nd Bot 3 0:16.000",
      "3rd You 0:16.200 (not finished)",
    ]);
    expect(container.querySelector("button[name=rematch]")?.textContent).toBe("Rematch");
    expect(container.querySelector("button[name=leave]")?.textContent).toBe("Leave");

    scene.dispose();
    expect(container.childElementCount).toBe(0);
  });

  it("hands Rematch and Leave to the caller", () => {
    const container = document.createElement("div");
    const pressed: string[] = [];
    createResultsScene(container, [], {
      rematch: () => pressed.push("rematch"),
      leave: () => pressed.push("leave"),
    });

    container.querySelector<HTMLButtonElement>("button[name=rematch]")?.click();
    container.querySelector<HTMLButtonElement>("button[name=leave]")?.click();
    expect(pressed).toEqual(["rematch", "leave"]);
  });
});
