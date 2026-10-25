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
    lapTimeS: 14,
    bestLapS: 14 + position,
    wrongWay: false,
    finishTimeS: 45 + position,
    ...overrides,
  };
}

/** Slot 0 is the local kart; slots 1 and 2 are bots; slot 1 won and slot 0 never finished. */
const standings: readonly Standing[] = [
  standing(3, { lap: 2, bestLapS: 16.2, finishTimeS: null }),
  standing(1),
  standing(2),
];
const isBot = (slot: number): boolean => slot > 0;
const results = { standings, localKart: 0, isBot };

describe("results rows", () => {
  it("lists every kart in finishing order with its name and best lap, unfinished karts included", () => {
    expect(resultsRows(results)).toEqual([
      { position: "1st", name: "Bot 2", bestLap: `0'15"000`, finished: true },
      { position: "2nd", name: "Bot 3", bestLap: `0'16"000`, finished: true },
      { position: "3rd", name: "You", bestLap: `0'16"200`, finished: false },
    ]);
  });

  it("names another human by the nickname the room gave, and by slot without one", () => {
    const humans = { ...results, isBot: (slot: number) => slot > 1 };
    expect(resultsRows(humans).map((row) => row.name)).toEqual(["Kart 2", "Bot 3", "You"]);
    const named = { ...humans, nickname: (slot: number) => (slot === 1 ? "ada" : undefined) };
    expect(resultsRows(named).map((row) => row.name)).toEqual(["ada", "Bot 3", "You"]);
  });

  it("leaves the best lap blank for a kart that completed none", () => {
    const rows = resultsRows({ ...results, standings: [standing(1, { bestLapS: null })] });
    expect(rows[0]?.bestLap).toBe(`-'--"---`);
  });
});

describe("Results Scene", () => {
  it("shows the rows in order and offers Rematch and Leave", () => {
    const container = document.createElement("div");
    const scene = createResultsScene(container, resultsRows(results), {
      rematch() {},
      leave() {},
    });

    const rows = Array.from(container.querySelectorAll("[data-result]")).map(
      (row) => row.textContent,
    );
    expect(rows).toEqual([
      `1st Bot 2 0'15"000`,
      `2nd Bot 3 0'16"000`,
      `3rd You 0'16"200 (not finished)`,
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
