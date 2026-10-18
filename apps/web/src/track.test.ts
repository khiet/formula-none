import { describe, expect, it } from "vitest";
import { DEFAULT_TRACK, selectTrack, TRACKS } from "./track";

describe("track selection from the query string", () => {
  it("defaults to Brands Patch", () => {
    expect(selectTrack("").name).toBe(DEFAULT_TRACK);
    expect(DEFAULT_TRACK).toBe("brands-patch");
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
