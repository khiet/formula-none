import { describe, expect, it } from "vitest";
import type { KartTransform } from "./bridge";
import { kartLabels } from "./kart-labels";

function at(position: readonly [number, number, number]): KartTransform {
  return {
    position,
    rotation: [0, 0, 0, 1],
    speed: 0,
    surface: "asphalt",
    drift: null,
    heldItem: null,
  };
}

const names = ["You", "ada", "Bot 3"];
const name = (kart: number): string => names[kart] ?? "";

describe("kart labels", () => {
  it("names every kart but the local one, floating over it", () => {
    const labels = kartLabels([at([0, 0.3, 0]), at([4, 0.3, -6]), at([-2, 0.3, 8])], 0, name);
    expect(labels.map(({ kart, text }) => ({ kart, text }))).toEqual([
      { kart: 1, text: "ada" },
      { kart: 2, text: "Bot 3" },
    ]);
    const [ada] = labels;
    expect(ada?.anchor[0]).toBe(4);
    expect(ada?.anchor[2]).toBe(-6);
    expect(ada?.anchor[1]).toBeGreaterThan(0.3);
  });

  it("follows each kart as it moves", () => {
    const before = kartLabels([at([0, 0.3, 0]), at([4, 0.3, -6])], 0, name);
    const after = kartLabels([at([0, 0.3, 0]), at([5, 1.3, -16])], 0, name);
    const rise = (before[0]?.anchor[1] ?? 0) - 0.3;
    expect(after[0]?.anchor).toEqual([5, 1.3 + rise, -16]);
  });
});
