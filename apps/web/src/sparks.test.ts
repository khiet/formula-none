import { describe, expect, it } from "vitest";
import type { Drift, MiniTurbo } from "./bridge";
import { sparkTell } from "./sparks";

function drifting(miniTurbo: MiniTurbo): Drift {
  return { direction: "right", chargeSeconds: 0, miniTurbo };
}

describe("spark tell", () => {
  it("shows no sparks off a drift", () => {
    expect(sparkTell(null)).toBeNull();
  });

  it("shows sparks through a drift and changes colour at blue and at orange", () => {
    const uncharged = sparkTell(drifting("none"));
    const blue = sparkTell(drifting("blue"));
    const orange = sparkTell(drifting("orange"));
    expect(uncharged).not.toBeNull();
    expect(blue?.colour).not.toBe(uncharged?.colour);
    expect(orange?.colour).not.toBe(blue?.colour);
    expect(orange?.colour).not.toBe(uncharged?.colour);
  });
});
