import { describe, expect, it } from "vitest";
import { CREW_HEIGHT_RANGE, CREW_PER_BOX, crewRoster } from "./crew-roster";
import { BROWS, FACIAL_HAIRS, HAIRS, LIVERIES, MOUTHS, NOSES, SKINS } from "./livery";

describe("the pit crew's roster", () => {
  it("draws seven faces per box from the head kit's pools, no hair style twice", () => {
    for (let slot = 0; slot < LIVERIES; slot += 1) {
      const crew = crewRoster(slot);
      expect(crew).toHaveLength(CREW_PER_BOX);
      expect(new Set(crew.map((face) => face.hair)).size).toBe(CREW_PER_BOX);
      for (const face of crew) {
        expect(HAIRS).toContain(face.hair);
        expect(FACIAL_HAIRS).toContain(face.facialHair);
        expect(Object.values(SKINS)).toContain(face.skin);
        expect(BROWS).toContain(face.brows);
        expect(NOSES).toContain(face.nose);
        expect(MOUTHS).toContain(face.mouth);
        expect(face.height).toBeGreaterThanOrEqual(CREW_HEIGHT_RANGE[0]);
        expect(face.height).toBeLessThanOrEqual(CREW_HEIGHT_RANGE[1]);
      }
    }
  });

  it("draws the same crew for a slot on every client, and another for another slot", () => {
    expect(crewRoster(3)).toEqual(crewRoster(3));
    expect(crewRoster(3)).not.toEqual(crewRoster(4));
  });
});
