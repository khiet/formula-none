import { describe, expect, it } from "vitest";
import { centrelineOf, DEFAULT_TRACK, selectTrack, TRACKS } from "./track";

describe("track selection from the query string", () => {
  it("defaults to Bell Deer", () => {
    expect(selectTrack("").name).toBe(DEFAULT_TRACK);
    expect(DEFAULT_TRACK).toBe("bell-deer");
  });

  it("loads the oval fixture on request", () => {
    expect(selectTrack("?track=oval").name).toBe("oval");
    expect(selectTrack("?debug=1&track=oval").name).toBe("oval");
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
