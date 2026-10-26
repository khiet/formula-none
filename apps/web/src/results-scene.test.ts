// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { Standing } from "./bridge";
import { livery, liveryCss } from "./livery";
import { createResultsScene, type RaceResults, resultsRows } from "./results-scene";

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

/** Eight karts finishing in slot order, with the local kart in `localKart`. */
function grid(localKart: number, overrides: Partial<Standing> = {}): RaceResults {
  return {
    standings: Array.from({ length: 8 }, (_, slot) =>
      standing(slot + 1, slot === localKart ? overrides : {}),
    ),
    localKart,
    isBot: (slot) => slot !== localKart,
  };
}

function mount(results: RaceResults) {
  const container = document.createElement("div");
  const scene = createResultsScene(container, resultsRows(results), {
    rematch() {},
    leave() {},
  });
  const text = (selector: string) => container.querySelector(selector)?.textContent ?? null;
  return { container, scene, text };
}

describe("results rows", () => {
  it("lists every kart in finishing order with its name, livery and best lap, unfinished karts included", () => {
    expect(resultsRows(results)).toEqual([
      {
        position: "1st",
        slot: 1,
        place: 1,
        name: "Bot 2",
        livery: liveryCss(1),
        bestLap: `0'15"000`,
        fastestLap: true,
        finished: true,
        you: false,
      },
      {
        position: "2nd",
        slot: 2,
        place: 2,
        name: "Bot 3",
        livery: liveryCss(2),
        bestLap: `0'16"000`,
        fastestLap: false,
        finished: true,
        you: false,
      },
      {
        position: "3rd",
        slot: 0,
        place: 3,
        name: "You",
        livery: liveryCss(0),
        bestLap: "Not finished",
        fastestLap: false,
        finished: false,
        you: true,
      },
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

  it("flags the lowest best lap wherever it finished, and every kart that ties it", () => {
    const tied = resultsRows({
      ...results,
      standings: [
        standing(1, { bestLapS: 15.5 }),
        standing(2, { bestLapS: 14.2 }),
        standing(3, { bestLapS: 14.2 }),
      ],
    });
    expect(tied.map((row) => row.fastestLap)).toEqual([false, true, true]);
  });

  it("gives the fastest lap only to a kart that finished, since one that did not shows no time", () => {
    const rows = resultsRows({
      ...results,
      standings: [
        standing(1, { bestLapS: 15.5 }),
        standing(2, { bestLapS: 15.1 }),
        standing(3, { bestLapS: 14.2, finishTimeS: null }),
      ],
    });
    expect(rows.map((row) => row.fastestLap)).toEqual([false, true, false]);
  });
});

describe("Results Scene", () => {
  it("cheers a win with confetti", () => {
    const { text, container } = mount(grid(0));
    expect(text("h1")).toBe("You win!");
    expect(container.querySelector("[data-confetti]")).not.toBeNull();
  });

  it("names the podium place for 2nd or 3rd", () => {
    const { text, container } = mount(grid(2));
    expect(text("h1")).toBe("Podium! 3rd");
    expect(container.querySelector("[data-confetti]")).not.toBeNull();
  });

  it("stamps the place from 4th down and counts the karts off the podium", () => {
    const sixth = mount(grid(5));
    expect(sixth.text("h1")).toBe("6th");
    expect(sixth.text("[data-subline]")).toBe("3 karts off the podium. One more go?");
    expect(sixth.container.querySelector("[data-confetti]")).toBeNull();
    expect(mount(grid(3)).text("[data-subline]")).toBe("1 kart off the podium. One more go?");
  });

  it("says the road ran out for a kart that did not finish, wherever it was placed", () => {
    const { text, container } = mount(grid(1, { finishTimeS: null }));
    expect(text("h1")).toBe("Out of road!");
    expect(container.querySelector("[data-confetti]")).toBeNull();
  });

  it("shows every kart in order with its livery and best lap, marking yours and the fastest lap", () => {
    const { container } = mount(results);
    const rows = Array.from(container.querySelectorAll<HTMLElement>("li[data-result]"));
    expect(rows.map((row) => row.dataset.result)).toEqual(["1st", "2nd", "3rd"]);
    const cell = (row: HTMLElement, name: string) =>
      row.querySelector(`[data-${name}]`)?.textContent ?? null;
    expect(rows.map((row) => cell(row, "name"))).toEqual(["Bot 2", "Bot 3", "You"]);
    expect(rows.map((row) => cell(row, "best-lap"))).toEqual([
      `0'15"000`,
      `0'16"000`,
      "Not finished",
    ]);
    expect(rows.map((row) => row.style.getPropertyValue("--livery"))).toEqual([
      liveryCss(1),
      liveryCss(2),
      liveryCss(0),
    ]);
    expect(rows.map((row) => row.hasAttribute("data-you"))).toEqual([false, false, true]);
    expect(rows.map((row) => cell(row, "fastest-lap"))).toEqual(["Fastest lap", null, null]);
  });

  it("puts the top three on the podium and the rest on the board", () => {
    const { container } = mount(grid(5));
    const places = (selector: string) =>
      Array.from(container.querySelectorAll<HTMLElement>(`${selector} li[data-result]`)).map(
        (row) => row.dataset.result,
      );
    expect(places("[data-podium]")).toEqual(["1st", "2nd", "3rd"]);
    expect(places("[data-board]")).toEqual(["4th", "5th", "6th", "7th", "8th"]);
  });

  it("paints each podium kart in its finisher's palette, with that driver in the seat", () => {
    const { container } = mount(grid(5));
    const css = (colour: number) => `#${colour.toString(16).padStart(6, "0")}`;
    const karts = Array.from(container.querySelectorAll<SVGElement>("[data-podium] .kart"));
    expect(karts).toHaveLength(3);
    karts.forEach((kart, slot) => {
      const paint = livery(slot);
      const colour = (name: string) => kart.style.getPropertyValue(`--${name}`);
      expect([colour("p"), colour("s"), colour("a")]).toEqual(
        [paint.primary, paint.secondary, paint.accent].map(css),
      );
      expect([colour("sk"), colour("hair")]).toEqual(
        [paint.driver.skin, paint.driver.hairColour].map(css),
      );
      expect(kart.dataset.hair).toBe(paint.driver.hair);
    });
  });

  it("offers Rematch and Leave, and takes itself down", () => {
    const { container, scene } = mount(results);
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
