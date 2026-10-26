// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { KartReadout, Phase, Standing } from "./bridge";
import {
  CONNECTION_LOST_BANNER,
  createInstrumentPanel,
  finishReadout,
  itemReadout,
  lapReadouts,
  phaseReadout,
} from "./instrument-panel";
import { kartReadout } from "./kart-readout-fixture";
import { hexCss, livery, liveryCss } from "./livery";
import { ON_THE_GRID, raceReadout } from "./race-readout-fixture";
import type { ItemSlot, RaceReadout } from "./race-run";

// A square loop, so the map's fit is easy to read: the grid faces -z.
const course = {
  centreline: [
    [0, 0, 0],
    [0, 0, -100],
    [100, 0, -100],
    [100, 0, 0],
  ],
  kartCount: 3,
  ownKart: 1,
} as const;

function at(position: readonly [number, number, number]): KartReadout {
  return kartReadout({ position });
}

// The sim's tuned top speed, which the speed cluster's gears are cut from.
const TOP_SPEED = 30;

const nameOf = (kart: number) => (kart === course.ownKart ? "You" : `Kart ${kart + 1}`);

const racing = { kind: "racing", elapsedSeconds: 5, capLeftSeconds: null } as const;

/** The race with the own kart, kart 1, on `standing` holding `itemSlot`, and the karts at `karts`. */
function race(
  standing: Standing,
  itemSlot: ItemSlot,
  karts: readonly KartReadout[] = [at([0, 0.3, 0]), at([0, 0.3, 0]), at([0, 0.3, 0])],
): RaceReadout {
  return raceReadout({
    karts,
    standings: [ON_THE_GRID, standing, ON_THE_GRID],
    phase: racing,
    itemSlot,
  });
}

describe("lap readouts", () => {
  it("shows the grid as first on lap one of three with the clocks blank", () => {
    expect(lapReadouts(ON_THE_GRID, 3)).toEqual({
      position: { numeral: "1", suffix: "st" },
      lap: "Lap 1/3",
      lapsCompleted: 0,
      lapTime: { label: "Time", value: `0'00"000` },
      bestLap: { label: "Best", value: `-'--"---` },
      wrongWay: "",
    });
  });

  it("prints the wrong-way warning only while the sim reports it", () => {
    expect(lapReadouts({ ...ON_THE_GRID, wrongWay: true }, 3).wrongWay).toBe("WRONG WAY");
    expect(lapReadouts(ON_THE_GRID, 3).wrongWay).toBe("");
  });

  it("shows position as an ordinal, the lap clock in minutes and the best lap once there is one", () => {
    const readouts = lapReadouts(
      { ...ON_THE_GRID, position: 3, lap: 2, lapTimeS: 75.4567, bestLapS: 14.9996 },
      3,
    );
    expect(readouts.position).toEqual({ numeral: "3", suffix: "rd" });
    expect(readouts.lap).toBe("Lap 2/3");
    expect(readouts.lapsCompleted).toBe(1);
    expect(readouts.lapTime).toEqual({ label: "Time", value: `1'15"457` });
    expect(readouts.bestLap).toEqual({ label: "Best", value: `0'15"000` });
  });

  it("counts every lap completed once the kart has finished, whatever lap it is on", () => {
    expect(lapReadouts({ ...ON_THE_GRID, lap: 3, finishTimeS: 70.5 }, 3).lapsCompleted).toBe(3);
    expect(lapReadouts({ ...ON_THE_GRID, lap: 4, finishTimeS: 70.5 }, 3).lapsCompleted).toBe(3);
  });
});

describe("the lap instruments on the page", () => {
  const mount = () => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(
      container,
      3,
      course,
      TOP_SPEED,
      nameOf,
      livery(course.ownKart),
    );
    const find = (name: string) =>
      container.querySelector<HTMLElement>(`[data-instrument="${name}"]`);
    return { panel, find };
  };

  it("print the position as a numeral with its suffix beside it, reading as one ordinal", () => {
    const { panel, find } = mount();
    panel.update(race({ ...ON_THE_GRID, position: 3 }, { item: null, firing: false }));
    const position = find("position");
    expect(position?.textContent).toBe("3rd");
    expect(position?.querySelector("[data-numeral]")?.textContent).toBe("3");
    expect(position?.querySelector("[data-suffix]")?.textContent).toBe("rd");
  });

  it("keep the lap count as text and fill one decorative flag per lap completed", () => {
    const { panel, find } = mount();
    const lap = find("lap");
    const flags = () => [...(lap?.querySelectorAll<HTMLElement>("[data-flag]") ?? [])];
    const filled = () => flags().filter((flag) => flag.dataset.flag === "done").length;
    panel.update(race(ON_THE_GRID, { item: null, firing: false }));
    expect(lap?.textContent).toBe("Lap 1/3");
    expect(flags()).toHaveLength(3);
    expect(filled()).toBe(0);
    expect(lap?.querySelector("[aria-hidden='true']")?.contains(flags()[0] ?? null)).toBe(true);
    panel.update(race({ ...ON_THE_GRID, lap: 3 }, { item: null, firing: false }));
    expect(lap?.textContent).toBe("Lap 3/3");
    expect(filled()).toBe(2);
    panel.update(race({ ...ON_THE_GRID, lap: 3, finishTimeS: 70 }, { item: null, firing: false }));
    expect(filled()).toBe(3);
  });

  it("print the clocks with their label words in the text", () => {
    const { panel, find } = mount();
    panel.update(
      race({ ...ON_THE_GRID, lapTimeS: 27.304, bestLapS: 44.91 }, { item: null, firing: false }),
    );
    expect(find("lap-time")?.textContent).toBe(`Time 0'27"304`);
    expect(find("best-lap")?.textContent).toBe(`Best 0'44"910`);
  });
});

/** A standing over the line on the last lap, at `position` after `finishTimeS`. */
const finished = (position: number, finishTimeS: number): Standing => ({
  ...ON_THE_GRID,
  lap: 3,
  position,
  finishTimeS,
});

describe("finish readout", () => {
  it("raises the banner over a kart still racing once the race has ended, with its final place", () => {
    const standings = [finished(1, 50), { ...ON_THE_GRID, position: 2 }];
    expect(finishReadout(standings, { kind: "outro" }, 1, nameOf)).toEqual({
      banner: "FINISH",
      place: "2nd",
      rows: [{ position: "1st", name: "Kart 1", time: `0'50"000` }],
    });
  });

  it("is nothing while the own kart is still racing", () => {
    expect(
      finishReadout([finished(1, 50), ON_THE_GRID, ON_THE_GRID], racing, 1, nameOf),
    ).toBeNull();
  });

  it("puts the own place under the banner and lists only the finishers, in finishing order", () => {
    const standings = [finished(2, 55.5), finished(1, 50), { ...ON_THE_GRID, position: 3 }];
    expect(finishReadout(standings, racing, 0, nameOf)).toEqual({
      banner: "FINISH",
      place: "2nd",
      rows: [
        { position: "1st", name: "You", time: `0'50"000` },
        { position: "2nd", name: "Kart 1", time: `0'55"500` },
      ],
    });
  });
});

describe("the finish on the page", () => {
  const mount = () => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(
      container,
      3,
      course,
      TOP_SPEED,
      nameOf,
      livery(course.ownKart),
    );
    const find = (name: string) =>
      container.querySelector<HTMLElement>(`[data-instrument="${name}"]`);
    const rows = () =>
      [...(find("standings")?.querySelectorAll("li") ?? [])].map((row) => row.textContent);
    const show = (standings: readonly Standing[], phase: Phase = racing) =>
      panel.update(
        raceReadout({ karts: [at([0, 0, 0]), at([0, 0, 0]), at([0, 0, 0])], standings, phase }),
      );
    return { find, rows, show };
  };
  it("keeps the banner and the standings down and the lap up until the own kart finishes or the race ends", () => {
    const { find, show } = mount();
    show([finished(1, 50), { ...ON_THE_GRID, position: 2, lap: 3 }, ON_THE_GRID]);
    expect(find("finish")?.hidden).toBe(true);
    expect(find("standings")?.hidden).toBe(true);
    expect(find("lap")?.hidden).toBe(false);
    show([finished(1, 50), { ...ON_THE_GRID, position: 2, lap: 3 }, ON_THE_GRID], {
      kind: "outro",
    });
    expect(find("finish")?.textContent).toBe("FINISH 2nd");
    expect(find("lap")?.hidden).toBe(true);
  });

  it("raises the banner with the place in place of the lap, and fills the standings as karts finish", () => {
    const { find, rows, show } = mount();
    show([finished(1, 50), finished(2, 52.25), { ...ON_THE_GRID, position: 3 }]);
    expect(find("finish")?.hidden).toBe(false);
    expect(find("finish")?.textContent).toBe("FINISH 2nd");
    expect(find("lap")?.hidden).toBe(true);
    expect(find("standings")?.hidden).toBe(false);
    expect(rows()).toEqual([`1st Kart 1 0'50"000`, `2nd You 0'52"250`]);

    show([finished(1, 50), finished(2, 52.25), finished(3, 61)]);
    expect(rows()).toEqual([`1st Kart 1 0'50"000`, `2nd You 0'52"250`, `3rd Kart 3 1'01"000`]);
  });
});

describe("the speed cluster on the page", () => {
  const mount = () => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(
      container,
      3,
      course,
      TOP_SPEED,
      nameOf,
      livery(course.ownKart),
    );
    const find = (name: string) =>
      container.querySelector<HTMLElement>(`[data-instrument="${name}"]`);
    const drive = (speed: number) =>
      panel.update(
        race(ON_THE_GRID, { item: null, firing: false }, [
          at([0, 0.3, 0]),
          kartReadout({ speed }),
          at([0, 0.3, 0]),
        ]),
      );
    return { container, find, drive };
  };

  it("reads the speed in km/h, the gear and the tyres as text", () => {
    const { find, drive } = mount();
    expect(find("speed")?.textContent).toBe("0 km/h");
    expect(find("gear")?.textContent).toBe("N");
    expect(find("tyre")?.textContent).toBe("100%");
    drive(23.6);
    expect(find("speed")?.textContent).toBe("85 km/h");
    expect(find("gear")?.textContent).toBe("5");
    drive(-5);
    expect(find("speed")?.textContent).toBe("18 km/h");
    expect(find("gear")?.textContent).toBe("R");
  });

  it("hides the shift lights and the kart icon from assistive technology", () => {
    const { container, find } = mount();
    const lights = container.querySelector("[data-shift-lights]");
    expect(lights?.getAttribute("aria-hidden")).toBe("true");
    expect(lights?.children).toHaveLength(12);
    expect(find("tyre")?.querySelector("svg")?.getAttribute("aria-hidden")).toBe("true");
  });

  it("lights the shift lights with the revs and marks the limiter on the gear so it can flash", () => {
    const { container, find, drive } = mount();
    const lights = () =>
      [...container.querySelectorAll<HTMLElement>("[data-shift-light]")].map(
        (light) => light.dataset.shiftLight,
      );
    expect(lights()).toEqual(Array(12).fill("off"));
    drive(42);
    expect(find("gear")?.textContent).toBe("6");
    expect(find("gear")?.dataset.limiter).toBe("true");
    expect(lights()).toEqual(Array(12).fill("blue"));
    drive(22);
    expect(find("gear")?.dataset.limiter).toBe("false");
    expect(lights()).toEqual([...Array(2).fill("green"), ...Array(10).fill("off")]);
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
    expect(phaseReadout({ kind: "racing", elapsedSeconds: 0, capLeftSeconds: null })).toBe("GO");
    expect(phaseReadout({ kind: "racing", elapsedSeconds: 0.9, capLeftSeconds: null })).toBe("GO");
    expect(phaseReadout({ kind: "racing", elapsedSeconds: 1, capLeftSeconds: null })).toBe("");
    expect(phaseReadout({ kind: "racing", elapsedSeconds: 40, capLeftSeconds: 12 })).toBe("");
    expect(phaseReadout({ kind: "outro" })).toBe("");
    expect(phaseReadout({ kind: "finished" })).toBe("");
  });
});

describe("the connection banner", () => {
  it("is down until the connection is lost, and down again once it is back", () => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(
      container,
      3,
      course,
      TOP_SPEED,
      nameOf,
      livery(course.ownKart),
    );
    const banner = container.querySelector<HTMLElement>('[data-instrument="connection-banner"]');
    expect(banner?.textContent).toBe(CONNECTION_LOST_BANNER);
    expect(banner?.hidden).toBe(true);
    panel.connectionLost(true);
    expect(banner?.hidden).toBe(false);
    panel.connectionLost(false);
    expect(banner?.hidden).toBe(true);
  });
});

describe("the item slot", () => {
  it("names the item held, in a state that says whether it is being fired", () => {
    expect(itemReadout({ item: null, firing: false })).toEqual({ text: "", state: "empty" });
    expect(itemReadout({ item: "boost", firing: false })).toEqual({ text: "Boost", state: "held" });
    expect(itemReadout({ item: "hazard", firing: false })).toEqual({
      text: "Hazard",
      state: "held",
    });
    expect(itemReadout({ item: "homing", firing: true })).toEqual({
      text: "Homing",
      state: "firing",
    });
    expect(itemReadout({ item: null, firing: true })).toEqual({ text: "", state: "firing" });
  });

  it("shows the slot the run reports on every update", () => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(
      container,
      3,
      course,
      TOP_SPEED,
      nameOf,
      livery(course.ownKart),
    );
    const slot = container.querySelector<HTMLElement>('[data-instrument="item"]');
    expect(slot?.textContent).toBe("");
    expect(slot?.dataset.itemState).toBe("empty");
    panel.update(race(ON_THE_GRID, { item: "boost", firing: false }));
    expect(slot?.textContent).toBe("Boost");
    expect(slot?.dataset.itemState).toBe("held");
    panel.update(race(ON_THE_GRID, { item: "boost", firing: true }));
    expect(slot?.dataset.itemState).toBe("firing");
    panel.update(race(ON_THE_GRID, { item: null, firing: false }));
    expect(slot?.textContent).toBe("");
    expect(slot?.dataset.itemState).toBe("empty");
  });

  it.each([
    ["boost", "Boost"],
    ["hazard", "Hazard"],
    ["homing", "Homing"],
  ] as const)("draws the %s sprite, named by data-item, with %s still its text", (item, name) => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(
      container,
      3,
      course,
      TOP_SPEED,
      nameOf,
      livery(course.ownKart),
    );
    const slot = container.querySelector<HTMLElement>('[data-instrument="item"]');
    expect(slot?.dataset.item).toBeUndefined();
    expect(slot?.querySelector("svg")).toBeNull();
    panel.update(race(ON_THE_GRID, { item, firing: false }));
    expect(slot?.dataset.item).toBe(item);
    const sprite = slot?.querySelector("svg");
    expect(sprite?.getAttribute("aria-hidden")).toBe("true");
    expect(sprite?.querySelectorAll("polygon").length).toBeGreaterThan(0);
    expect(slot?.textContent).toBe(name);
    panel.update(race(ON_THE_GRID, { item, firing: true }));
    expect(slot?.querySelector("svg")).toBe(sprite);
    panel.update(race(ON_THE_GRID, { item: null, firing: false }));
    expect(slot?.dataset.item).toBeUndefined();
    expect(slot?.querySelector("svg")).toBeNull();
  });

  it("swaps the sprite when the server hands the kart a different item", () => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(
      container,
      3,
      course,
      TOP_SPEED,
      nameOf,
      livery(course.ownKart),
    );
    const slot = container.querySelector<HTMLElement>('[data-instrument="item"]');
    panel.update(race(ON_THE_GRID, { item: "boost", firing: false }));
    panel.update(race(ON_THE_GRID, { item: "hazard", firing: false }));
    expect(slot?.dataset.item).toBe("hazard");
    expect(slot?.textContent).toBe("Hazard");
    const sprites = slot?.querySelectorAll("svg") ?? [];
    expect(sprites).toHaveLength(1);
    expect(sprites[0]?.dataset.sprite).toBe("hazard");
  });
});

describe("the course map", () => {
  // A marker is drawn around its own origin and moved by its transform.
  const markerAt = (marker: Element | null | undefined): [number, number] => {
    const moved = /translate\(([-\d.]+) ([-\d.]+)\)/.exec(marker?.getAttribute("transform") ?? "");
    return [Number(moved?.[1]), Number(moved?.[2])];
  };
  const extent = (marker: Element | null | undefined) =>
    Math.max(
      ...[...(marker?.querySelectorAll("polygon") ?? [])].flatMap((shape) =>
        (shape.getAttribute("points") ?? "").split(/[ ,]/).map((n) => Math.abs(Number(n))),
      ),
    );

  it("outlines the track from the centreline with a start tick", () => {
    const container = document.createElement("div");
    createInstrumentPanel(container, 3, course, TOP_SPEED, nameOf, livery(course.ownKart));
    const map = container.querySelector<SVGSVGElement>('svg[data-instrument="course-map"]');
    expect(map).not.toBeNull();
    const outline = map?.querySelector("path");
    expect(outline?.getAttribute("d")).toMatch(/^M .* Z$/);
    expect(outline?.getAttribute("fill")).toBe("none");
    expect(map?.querySelector("line")).not.toBeNull();
  });

  it("moves a kart's marker with its transform, down the map as it drives along +z", () => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(
      container,
      3,
      course,
      TOP_SPEED,
      nameOf,
      livery(course.ownKart),
    );
    const own = at([50, 0.3, -50]);
    const standing = ON_THE_GRID;
    const slot = { item: null, firing: false };
    panel.update(race(standing, slot, [at([0, 0.3, 0]), own, at([100, 0.3, -100])]));
    const marker = container.querySelector('[data-instrument="course-map"] [data-kart="2"]');
    const [u0, v0] = markerAt(marker);
    panel.update(race(standing, slot, [at([0, 0.3, 0]), own, at([100, 0.3, -40])]));
    const [u1, v1] = markerAt(marker);
    expect(u1).toBe(u0);
    expect(v1).toBeGreaterThan(v0);
    // The own kart sits at the centre of the square, so at the map's centre.
    const [ownU, ownV] = markerAt(
      container.querySelector('[data-instrument="course-map"] [data-kart="1"]'),
    );
    expect(ownU).toBeCloseTo(50);
    expect(ownV).toBeCloseTo(50);
  });

  it("marks every kart in its livery, the own kart larger, ringed and on top", () => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(
      container,
      3,
      course,
      TOP_SPEED,
      nameOf,
      livery(course.ownKart),
    );
    panel.update(race(ON_THE_GRID, { item: null, firing: false }));
    const markers = [
      ...container.querySelectorAll<SVGGElement>('[data-instrument="course-map"] [data-kart]'),
    ];
    expect(markers.map((marker) => marker.dataset.kart)).toEqual(["0", "2", "1"]);
    expect(markers.map((marker) => marker.getAttribute("fill"))).toEqual([
      liveryCss(0),
      liveryCss(2),
      liveryCss(1),
    ]);
    const ownMarker = container.querySelector<SVGGElement>(
      '[data-instrument="course-map"] [data-own]',
    );
    expect(ownMarker?.dataset.kart).toBe("1");
    expect(ownMarker?.querySelector("[data-ring]")?.getAttribute("fill")).toBe("#ffffff");
    expect(markers[0]?.querySelector("[data-ring]")).toBeNull();
    expect(extent(ownMarker)).toBeGreaterThan(extent(markers[0]));
    expect(container.querySelectorAll('[data-instrument="course-map"] [data-own]')).toHaveLength(1);
  });
});

describe("the panel in the local kart's livery", () => {
  const fills = (element: Element | null | undefined) =>
    [...(element?.querySelectorAll("[fill]") ?? [])].map((shape) => shape.getAttribute("fill"));

  it.each([
    [0, 3],
    [3, 0],
  ])("paints slot %i's position plate, lap flags and tyre gauge, not slot %i's", (slot, other) => {
    const paint = livery(slot);
    const container = document.createElement("div");
    createInstrumentPanel(container, 3, course, TOP_SPEED, nameOf, paint);
    const find = (name: string) =>
      container.querySelector<HTMLElement>(`[data-instrument="${name}"]`);

    expect(find("position")?.style.getPropertyValue("--plate")).toBe(hexCss(paint.primary));
    const flags = fills(find("lap")?.querySelector("[data-flags]"));
    expect(flags).toContain(hexCss(paint.accent));
    expect(flags).not.toContain(hexCss(livery(other).accent));
    const tyre = fills(find("tyre")?.querySelector("svg"));
    expect(tyre).toEqual(
      expect.arrayContaining([
        hexCss(paint.primary),
        hexCss(paint.secondary),
        hexCss(paint.accent),
      ]),
    );
    expect(tyre).not.toContain(hexCss(livery(other).primary));
  });
});
