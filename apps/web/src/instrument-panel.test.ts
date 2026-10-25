// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { KartReadout, Standing } from "./bridge";
import {
  CONNECTION_LOST_BANNER,
  createInstrumentPanel,
  itemReadout,
  lapReadouts,
  phaseReadout,
} from "./instrument-panel";
import { kartReadout } from "./kart-readout-fixture";
import { liveryCss } from "./livery";

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
      position: { numeral: "1", suffix: "st" },
      lap: "Lap 1/3",
      lapsCompleted: 0,
      lapTime: { label: "Time", value: `0'00"000` },
      bestLap: { label: "Best", value: `-'--"---` },
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
    expect(readouts.position).toEqual({ numeral: "3", suffix: "rd" });
    expect(readouts.lap).toBe("Lap 2/3");
    expect(readouts.lapsCompleted).toBe(1);
    expect(readouts.lapTime).toEqual({ label: "Time", value: `1'15"457` });
    expect(readouts.bestLap).toEqual({ label: "Best", value: `0'15"000` });
  });

  it("counts every lap completed once the kart has finished, whatever lap it is on", () => {
    expect(lapReadouts({ ...onTheGrid, lap: 3, finishTimeSeconds: 70.5 }, 3).lapsCompleted).toBe(3);
    expect(lapReadouts({ ...onTheGrid, lap: 4, finishTimeSeconds: 70.5 }, 3).lapsCompleted).toBe(3);
  });
});

describe("the lap instruments on the page", () => {
  const racing = { kind: "racing", elapsedSeconds: 5, graceLeftSeconds: null } as const;
  const mount = () => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(container, 3, course);
    const find = (name: string) =>
      container.querySelector<HTMLElement>(`[data-instrument="${name}"]`);
    return { panel, find };
  };

  it("print the position as a numeral with its suffix beside it, reading as one ordinal", () => {
    const { panel, find } = mount();
    const local = at([0, 0.3, 0]);
    panel.update(local, { ...onTheGrid, position: 3 }, racing, { item: null, firing: false }, [
      local,
    ]);
    const position = find("position");
    expect(position?.textContent).toBe("3rd");
    expect(position?.querySelector("[data-numeral]")?.textContent).toBe("3");
    expect(position?.querySelector("[data-suffix]")?.textContent).toBe("rd");
  });

  it("keep the lap count as text and fill one decorative flag per lap completed", () => {
    const { panel, find } = mount();
    const local = at([0, 0.3, 0]);
    const lap = find("lap");
    const flags = () => [...(lap?.querySelectorAll<HTMLElement>("[data-flag]") ?? [])];
    const filled = () => flags().filter((flag) => flag.dataset.flag === "done").length;
    panel.update(local, onTheGrid, racing, { item: null, firing: false }, [local]);
    expect(lap?.textContent).toBe("Lap 1/3");
    expect(flags()).toHaveLength(3);
    expect(filled()).toBe(0);
    expect(lap?.querySelector("[aria-hidden='true']")?.contains(flags()[0] ?? null)).toBe(true);
    panel.update(local, { ...onTheGrid, lap: 3 }, racing, { item: null, firing: false }, [local]);
    expect(lap?.textContent).toBe("Lap 3/3");
    expect(filled()).toBe(2);
    panel.update(
      local,
      { ...onTheGrid, lap: 3, finishTimeSeconds: 70 },
      racing,
      { item: null, firing: false },
      [local],
    );
    expect(filled()).toBe(3);
  });

  it("print the clocks with their label words in the text", () => {
    const { panel, find } = mount();
    const local = at([0, 0.3, 0]);
    panel.update(
      local,
      { ...onTheGrid, lapTimeSeconds: 27.304, bestLapSeconds: 44.91 },
      racing,
      { item: null, firing: false },
      [local],
    );
    expect(find("lap-time")?.textContent).toBe(`Time 0'27"304`);
    expect(find("best-lap")?.textContent).toBe(`Best 0'44"910`);
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
    const panel = createInstrumentPanel(container, 3, course);
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
    const panel = createInstrumentPanel(container, 3, course);
    const slot = container.querySelector<HTMLElement>('[data-instrument="item"]');
    const local = at([0, 0.3, 0]);
    const racing = { kind: "racing", elapsedSeconds: 5, graceLeftSeconds: null } as const;
    expect(slot?.textContent).toBe("");
    expect(slot?.dataset.itemState).toBe("empty");
    panel.update(local, onTheGrid, racing, { item: "boost", firing: false }, [local]);
    expect(slot?.textContent).toBe("Boost");
    expect(slot?.dataset.itemState).toBe("held");
    panel.update(local, onTheGrid, racing, { item: "boost", firing: true }, [local]);
    expect(slot?.dataset.itemState).toBe("firing");
    panel.update(local, onTheGrid, racing, { item: null, firing: false }, [local]);
    expect(slot?.textContent).toBe("");
    expect(slot?.dataset.itemState).toBe("empty");
  });

  it.each([
    ["boost", "Boost"],
    ["hazard", "Hazard"],
    ["homing", "Homing"],
  ] as const)("draws the %s sprite, named by data-item, with %s still its text", (item, name) => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(container, 3, course);
    const slot = container.querySelector<HTMLElement>('[data-instrument="item"]');
    const local = at([0, 0.3, 0]);
    const racing = { kind: "racing", elapsedSeconds: 5, graceLeftSeconds: null } as const;
    expect(slot?.dataset.item).toBeUndefined();
    expect(slot?.querySelector("svg")).toBeNull();
    panel.update(local, onTheGrid, racing, { item, firing: false }, [local]);
    expect(slot?.dataset.item).toBe(item);
    const sprite = slot?.querySelector("svg");
    expect(sprite?.getAttribute("aria-hidden")).toBe("true");
    expect(sprite?.querySelectorAll("rect").length).toBeGreaterThan(0);
    expect(slot?.textContent).toBe(name);
    panel.update(local, onTheGrid, racing, { item, firing: true }, [local]);
    expect(slot?.querySelector("svg")).toBe(sprite);
    panel.update(local, onTheGrid, racing, { item: null, firing: false }, [local]);
    expect(slot?.dataset.item).toBeUndefined();
    expect(slot?.querySelector("svg")).toBeNull();
  });

  it("swaps the sprite when the server hands the kart a different item", () => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(container, 3, course);
    const slot = container.querySelector<HTMLElement>('[data-instrument="item"]');
    const local = at([0, 0.3, 0]);
    const racing = { kind: "racing", elapsedSeconds: 5, graceLeftSeconds: null } as const;
    panel.update(local, onTheGrid, racing, { item: "boost", firing: false }, [local]);
    panel.update(local, onTheGrid, racing, { item: "hazard", firing: false }, [local]);
    expect(slot?.dataset.item).toBe("hazard");
    expect(slot?.textContent).toBe("Hazard");
    const sprites = slot?.querySelectorAll("svg") ?? [];
    expect(sprites).toHaveLength(1);
    expect(sprites[0]?.dataset.sprite).toBe("hazard");
  });
});

describe("the course map", () => {
  const racing = { kind: "racing", elapsedSeconds: 5, graceLeftSeconds: null } as const;
  const markerAt = (marker: Element | null | undefined): [number, number] => [
    Number(marker?.getAttribute("cx")),
    Number(marker?.getAttribute("cy")),
  ];

  it("outlines the track from the centreline with a start tick", () => {
    const container = document.createElement("div");
    createInstrumentPanel(container, 3, course);
    const map = container.querySelector<SVGSVGElement>('svg[data-instrument="course-map"]');
    expect(map).not.toBeNull();
    const outline = map?.querySelector("path");
    expect(outline?.getAttribute("d")).toMatch(/^M .* Z$/);
    expect(outline?.getAttribute("fill")).toBe("none");
    expect(map?.querySelector("line")).not.toBeNull();
  });

  it("moves a kart's marker with its transform, down the map as it drives along +z", () => {
    const container = document.createElement("div");
    const panel = createInstrumentPanel(container, 3, course);
    const own = at([50, 0.3, -50]);
    const standing = onTheGrid;
    panel.update(own, standing, racing, { item: null, firing: false }, [
      at([0, 0.3, 0]),
      own,
      at([100, 0.3, -100]),
    ]);
    const marker = container.querySelector('[data-instrument="course-map"] [data-kart="2"]');
    const [u0, v0] = markerAt(marker);
    panel.update(own, standing, racing, { item: null, firing: false }, [
      at([0, 0.3, 0]),
      own,
      at([100, 0.3, -40]),
    ]);
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
    const panel = createInstrumentPanel(container, 3, course);
    const own = at([0, 0.3, 0]);
    panel.update(own, onTheGrid, racing, { item: null, firing: false }, [own, own, own]);
    const markers = [
      ...container.querySelectorAll<SVGCircleElement>('[data-instrument="course-map"] [data-kart]'),
    ];
    expect(markers.map((marker) => marker.dataset.kart)).toEqual(["0", "2", "1"]);
    expect(markers.map((marker) => marker.getAttribute("fill"))).toEqual([
      liveryCss(0),
      liveryCss(2),
      liveryCss(1),
    ]);
    const ownMarker = container.querySelector<SVGCircleElement>(
      '[data-instrument="course-map"] [data-own]',
    );
    expect(ownMarker?.dataset.kart).toBe("1");
    expect(ownMarker?.getAttribute("stroke")).toBe("#fff");
    const radius = (marker: Element | null) => Number(marker?.getAttribute("r"));
    expect(radius(ownMarker)).toBeGreaterThan(radius(markers[0] ?? null));
    expect(container.querySelectorAll('[data-instrument="course-map"] [data-own]')).toHaveLength(1);
  });
});
