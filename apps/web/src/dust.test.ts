import { describe, expect, it } from "vitest";
import type { KartTransform, Surface } from "./bridge";
import { dustTell } from "./dust";

function kartOn(surface: Surface, speed = 10): KartTransform {
  return {
    position: [0, 0, 0],
    rotation: [0, 0, 0, 1],
    speed,
    surface,
    drift: null,
    heldItem: null,
  };
}

describe("dust tell", () => {
  it("puffs over sand and grass in different colours and not over the track", () => {
    const sand = dustTell(kartOn("sand"));
    const grass = dustTell(kartOn("grass"));
    expect(sand).not.toBeNull();
    expect(grass).not.toBeNull();
    expect(sand?.colour).not.toBe(grass?.colour);
    expect(dustTell(kartOn("asphalt"))).toBeNull();
    expect(dustTell(kartOn("wall"))).toBeNull();
    expect(dustTell(kartOn("out of world"))).toBeNull();
  });

  it("puffs whichever way the kart moves and not at a standstill", () => {
    expect(dustTell(kartOn("sand", -5))).not.toBeNull();
    expect(dustTell(kartOn("sand", 0))).toBeNull();
  });
});
