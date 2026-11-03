import { describe, expect, it } from "vitest";
import {
  BOT_LEVELS,
  botLevelFromSearch,
  COURSES,
  centrelineOf,
  DEFAULT_TRACK,
  loadBotLevel,
  loadCoursePick,
  saveBotLevel,
  saveCoursePick,
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

describe("bot level from the query string", () => {
  it("reads ?bots= for a solo race and ignores anything else", () => {
    expect(botLevelFromSearch("?bots=ace")).toBe("ace");
    expect(botLevelFromSearch("?track=oval&bots=pro")).toBe("pro");
    expect(botLevelFromSearch("?bots=wild")).toBeUndefined();
    expect(botLevelFromSearch("")).toBeUndefined();
  });
});
