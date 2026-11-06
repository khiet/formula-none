import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { Picks } from "./generated/sim";
import { pickOffer } from "./picks";
import {
  COURSES,
  centrelineOf,
  DEFAULT_TRACK,
  loadCoursePick,
  loadPicks,
  picksFromSearch,
  pitLaneOf,
  saveCoursePick,
  savePicks,
  selectTrack,
  TRACKS,
} from "./track";

describe("track selection from the query string", () => {
  it("defaults to Bell Deer", () => {
    expect(selectTrack("").name).toBe(DEFAULT_TRACK);
    expect(DEFAULT_TRACK).toBe("bell-deer");
  });

  it("loads the oval fixture on request", () => {
    expect(selectTrack("?track=oval").name).toBe("oval");
    expect(selectTrack("?debug=1&track=oval").name).toBe("oval");
  });

  it("loads Goldstone on request", () => {
    expect(selectTrack("?track=goldstone").name).toBe("goldstone");
  });

  it("loads Monarch on request", () => {
    expect(selectTrack("?track=monarch").name).toBe("monarch");
  });

  it("falls back to the default for an unknown track", () => {
    expect(selectTrack("?track=mushroom-cup").name).toBe(DEFAULT_TRACK);
    expect(selectTrack("?track=toString").name).toBe(DEFAULT_TRACK);
  });

  it("names a glTF and a sidecar for every track", () => {
    for (const track of Object.values(TRACKS)) {
      expect(track.gltfUrl).toMatch(/\.glb$/);
      expect(track.sidecarUrl).toMatch(/\.track\.json$/);
    }
  });
});

describe("the centreline from a sidecar", () => {
  it("reads the points in the sidecar's order", () => {
    const sidecar = JSON.stringify({
      version: 1,
      name: "oval",
      centreline: [
        [30, 0, 0],
        [30, 0, -1],
        [29.5, 0.2, -2],
      ],
      checkpoints: [],
    });
    expect(centrelineOf(sidecar)).toEqual([
      [30, 0, 0],
      [30, 0, -1],
      [29.5, 0.2, -2],
    ]);
  });

  it("rejects a sidecar without a centreline", () => {
    expect(() => centrelineOf(JSON.stringify({ version: 1, name: "oval" }))).toThrow(/centreline/);
    expect(() => centrelineOf(JSON.stringify({ centreline: [[1, 2]] }))).toThrow(/centreline/);
  });
});

describe("reading a sidecar's pit lane", () => {
  it("gives the path and the pit boxes of a course's lane, and nothing for a course without one", () => {
    const path = [
      [1, 0, 2],
      [3, 0, 4],
    ];
    const pit_boxes = [{ position: [2, 0, 3], yaw: 0.5 }];
    expect(pitLaneOf(JSON.stringify({ pit_lane: { path, pit_boxes } }))?.path).toEqual(path);
    expect(
      pitLaneOf(JSON.stringify({ pit_lane: { path, pit_boxes } }))?.pitBoxes.map(
        ({ position, yaw }) => ({ position, yaw }),
      ),
    ).toEqual(pit_boxes);
    expect(pitLaneOf(JSON.stringify({ centreline: [] }))).toBeUndefined();
  });

  it("has the crew on the pit building's side of each lane, whichever side of the road it runs", () => {
    const sidecar = (name: string) =>
      readFileSync(
        new URL(`../../../assets/tracks/${name}/${name}.track.json`, import.meta.url),
        "utf8",
      );
    const sides = (name: string) =>
      new Set(pitLaneOf(sidecar(name))?.pitBoxes.map((box) => box.buildingSide));
    expect(sides("bell-deer")).toEqual(new Set([1]));
    expect(sides("goldstone")).toEqual(new Set([1]));
    expect(sides("monarch")).toEqual(new Set([1]));
  });

  it("finds Bell Deer's lane and none on the oval in the sidecars the sim reads", () => {
    const sidecar = (name: string) =>
      readFileSync(
        new URL(`../../../assets/tracks/${name}/${name}.track.json`, import.meta.url),
        "utf8",
      );
    const bellDeer = pitLaneOf(sidecar("bell-deer"));
    expect(bellDeer?.path.length).toBeGreaterThan(100);
    expect(bellDeer?.pitBoxes).toHaveLength(8);
    expect(pitLaneOf(sidecar("oval"))).toBeUndefined();
  });
});

/** A Storage over a map; `refuses` makes every call throw, as a blocked store does. */
function storage(refuses = false): Storage {
  const items = new Map<string, string>();
  const check = () => {
    if (refuses) throw new DOMException("blocked", "SecurityError");
  };
  return {
    get length() {
      return items.size;
    },
    clear: () => items.clear(),
    key: (index) => [...items.keys()][index] ?? null,
    getItem: (key) => {
      check();
      return items.get(key) ?? null;
    },
    setItem: (key, value) => {
      check();
      items.set(key, value);
    },
    removeItem: (key) => items.delete(key),
  };
}

describe("the remembered course", () => {
  it("is Bell Deer on a first visit and the last pick after one", () => {
    const store = storage();
    expect(loadCoursePick(store).track).toBe("bell-deer");
    const goldstone = COURSES.find((course) => course.track === "goldstone");
    if (goldstone === undefined) throw new Error("no Goldstone");
    saveCoursePick(store, goldstone);
    expect(loadCoursePick(store)).toBe(goldstone);
  });

  it("falls back to Bell Deer for a saved track that is no course, and for storage that refuses", () => {
    const store = storage();
    store.setItem("course", "oval");
    expect(loadCoursePick(store).track).toBe("bell-deer");
    store.setItem("course", "monza");
    expect(loadCoursePick(store).track).toBe("bell-deer");
    const blocked = storage(true);
    expect(loadCoursePick(blocked).track).toBe("bell-deer");
    expect(() => saveCoursePick(blocked, loadCoursePick(blocked))).not.toThrow();
  });
});

// The offer as the Title Scene makes it, from the sim's choices.
const OFFER = pickOffer({ level: ["basic", "pro", "ace"], tyres: ["off", "on"], laps: [3, 5] });
const FRESH: Picks = { level: "basic", tyres: "on", laps: 3 };

describe("the remembered picks", () => {
  it("are the fresh ones until saved, then what was saved", () => {
    const store = storage();
    expect(loadPicks(store, OFFER, FRESH)).toEqual(FRESH);
    const picked: Picks = { level: "ace", tyres: "off", laps: 5 };
    savePicks(store, OFFER, picked);
    expect(loadPicks(store, OFFER, FRESH)).toEqual(picked);
  });

  it("are kept one key a pick, the keys the query string asks by", () => {
    const store = storage();
    savePicks(store, OFFER, { level: "pro", tyres: "off", laps: 5 });
    expect([store.getItem("bots"), store.getItem("tyres"), store.getItem("laps")]).toEqual([
      "pro",
      "off",
      "5",
    ]);
  });

  it("fall back pick by pick to the fresh one for a saved value no creator is offered", () => {
    const store = storage();
    store.setItem("bots", "wild");
    store.setItem("tyres", "off");
    store.setItem("laps", "4");
    expect(loadPicks(store, OFFER, FRESH)).toEqual({ ...FRESH, tyres: "off" });
  });

  it("are the fresh ones from storage that refuses, and saving to it is let be", () => {
    const blocked = storage(true);
    expect(loadPicks(blocked, OFFER, FRESH)).toEqual(FRESH);
    expect(() => savePicks(blocked, OFFER, FRESH)).not.toThrow();
  });
});

describe("picks from the query string", () => {
  it("reads ?bots=, ?tyres= and ?laps= for a solo race beside ?track=", () => {
    expect(picksFromSearch("?track=oval&bots=ace&tyres=off&laps=5", OFFER)).toEqual({
      level: "ace",
      tyres: "off",
      laps: 5,
    });
    expect(picksFromSearch("?bots=pro", OFFER)).toEqual({ level: "pro" });
  });

  it("leaves out a pick asked for as nothing a creator is offered, and any not asked for", () => {
    expect(picksFromSearch("?bots=wild&tyres=slick&laps=4", OFFER)).toEqual({});
    expect(picksFromSearch("", OFFER)).toEqual({});
  });
});
