import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  BOT_LEVELS,
  botLevelFromSearch,
  COURSES,
  centrelineOf,
  DEFAULT_TRACK,
  lapsFromSearch,
  loadBotLevel,
  loadCoursePick,
  loadLaps,
  loadTyres,
  pitLaneOf,
  saveBotLevel,
  saveCoursePick,
  saveLaps,
  saveTyres,
  selectTrack,
  TRACKS,
  tyresFromSearch,
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
    expect(sides("goldstone")).toEqual(new Set([-1]));
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

describe("the remembered bot level", () => {
  it("is Basic until saved, then what was saved", () => {
    const store = storage();
    expect(loadBotLevel(store)).toBe("basic");
    saveBotLevel(store, "ace");
    expect(loadBotLevel(store)).toBe("ace");
  });

  it("falls back to Basic for a saved level that is none, and for storage that refuses", () => {
    const store = storage();
    store.setItem("bots", "wild");
    expect(loadBotLevel(store)).toBe("basic");
    const blocked = storage(true);
    expect(loadBotLevel(blocked)).toBe("basic");
    expect(() => saveBotLevel(blocked, "pro")).not.toThrow();
  });

  it("describes every level in order, Basic first, for the Title Scene's row", () => {
    expect(BOT_LEVELS.map((level) => level.level)).toEqual(["basic", "pro", "ace"]);
    expect(BOT_LEVELS.map((level) => level.name)).toEqual(["Basic", "Pro", "Ace"]);
    for (const level of BOT_LEVELS) expect(level.blurb).toMatch(/\S/);
  });
});

describe("the laps pick", () => {
  it("is the first choice until picked, and remembered once it is", () => {
    const store = storage();
    expect(loadLaps(store, [3, 5])).toBe(3);
    saveLaps(store, 5);
    expect(loadLaps(store, [3, 5])).toBe(5);
  });

  it("reads anything else saved as the first choice", () => {
    const store = storage();
    store.setItem("laps", "4");
    expect(loadLaps(store, [3, 5])).toBe(3);
  });

  it("reads ?laps= for a solo race and ignores anything else", () => {
    expect(lapsFromSearch("?laps=5", [3, 5])).toBe(5);
    expect(lapsFromSearch("?track=oval&laps=3", [3, 5])).toBe(3);
    expect(lapsFromSearch("?laps=4", [3, 5])).toBeUndefined();
    expect(lapsFromSearch("", [3, 5])).toBeUndefined();
  });
});

describe("the Tyres pick", () => {
  it("is On until saved, then what was saved", () => {
    const store = storage();
    expect(loadTyres(store)).toBe("on");
    saveTyres(store, "off");
    expect(loadTyres(store)).toBe("off");
  });

  it("falls back to On for a saved pick that is none, and for storage that refuses", () => {
    const store = storage();
    store.setItem("tyres", "slick");
    expect(loadTyres(store)).toBe("on");
    const blocked = storage(true);
    expect(loadTyres(blocked)).toBe("on");
    expect(() => saveTyres(blocked, "off")).not.toThrow();
  });

  it("reads ?tyres= for a solo race and ignores anything else", () => {
    expect(tyresFromSearch("?tyres=on")).toBe("on");
    expect(tyresFromSearch("?track=oval&tyres=off")).toBe("off");
    expect(tyresFromSearch("?tyres=slick")).toBeUndefined();
    expect(tyresFromSearch("")).toBeUndefined();
  });
});

describe("bot level from the query string", () => {
  it("reads ?bots= for a solo race and ignores anything else", () => {
    expect(botLevelFromSearch("?bots=ace")).toBe("ace");
    expect(botLevelFromSearch("?track=oval&bots=pro")).toBe("pro");
    expect(botLevelFromSearch("?bots=wild")).toBeUndefined();
    expect(botLevelFromSearch("")).toBeUndefined();
  });
});
